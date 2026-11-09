export const effects = [
  ['Fade', 'Black fade in/out', '◐'],
  ['Cross dissolve', 'Blend across a cut', '◫'],
  ['Sine Wave', 'Animated sine wave distortion', '≋'],
  ['Blur', 'Soft focus blur', '◌'],
  ['Soft glow', 'Dreamy highlights', '✦'],
  ['Warm film', 'Golden color tone', '☼'],
  ['Vignette', 'Soft edge shading', '◍'],
  ['Sharpen', 'Crisp detail', '✧'],
  ['Grain', 'Film grain texture', '▦'],
  ['VHS', 'Analog color and tracking', '▤'],
  ['Chromatic Aberration', 'Split color channels with adjustable offset', '◈'],
];

export const chromaticPalettes = {
  RGB: ['#ff0000', '#00ff00', '#0000ff'],
  GBY: ['#00ff66', '#168cff', '#ffe600'],
  'Pink / Blue / Purple': ['#ff4f9a', '#39d5ff', '#914dff'],
};

export function effectKeyframeProperties(effect) {
  if (effect.effectName === 'Chromatic Aberration') return ['intensity', 'offset'];
  if (effect.effectName === 'Sine Wave') return ['intensity', 'wavelength', 'waveSpeed'];
  return ['intensity'];
}

export function chromaticColors(effect) {
  if (effect.palette !== 'Custom') return chromaticPalettes[effect.palette] || chromaticPalettes.RGB;
  const saved = effect.chromaticColors || {};
  return [saved.red || '#ff0000', saved.green || '#00ff00', saved.blue || '#0000ff'];
}

export function normalizeEffectKeys(effect) {
  if (!Array.isArray(effect.keys)) effect.keys = [];
  const duration = Math.max(0, Number(effect.duration) || 0);
  const frames = new Map();
  for (const raw of effect.keys) {
    if (!raw || typeof raw !== 'object' || !Number.isFinite(Number(raw.time))) continue;
    const time = Math.round(Math.max(0, Math.min(duration, Number(raw.time))) * 30) / 30;
    const controls = Object.fromEntries(Object.entries(raw.controls || {}).filter(([, value]) => Number.isFinite(Number(value))));
    if (!Object.keys(controls).length) continue;
    const existing = frames.get(time);
    if (existing) existing.controls = { ...existing.controls, ...controls };
    else frames.set(time, { time, controls });
  }
  effect.keys = [...frames.values()].sort((a, b) => a.time - b.time);
  return effect.keys;
}

export function effectValueAt(effect, name, time, fallback = 50) {
  const base = Number.isFinite(Number(effect[name])) ? Number(effect[name]) : fallback;
  const keys = normalizeEffectKeys(effect).filter(key => Number.isFinite(Number(key.controls[name])));
  let previousTime = 0;
  let previousValue = base;
  for (const key of keys) {
    const value = Number(key.controls[name]);
    if (time < key.time) {
      const ratio = Math.max(0, Math.min(1, (time - previousTime) / Math.max(1 / 30, key.time - previousTime)));
      return previousValue + (value - previousValue) * ratio;
    }
    previousTime = key.time;
    previousValue = value;
  }
  return previousValue;
}

export function effectKeyframeAt(effect, time) {
  const position = Math.max(0, Math.min(Number(effect.duration) || 0, Number(time) || 0));
  return normalizeEffectKeys(effect).find(key => Math.abs(key.time - position) <= 0.001) || null;
}

export function setEffectValueAt(effect, time, name, value) {
  const frame = Math.round(Math.max(0, Math.min(Number(effect.duration) || 0, time)) * 30) / 30;
  const keys = normalizeEffectKeys(effect);
  let key = keys.find(item => Math.abs(item.time - frame) <= 0.001);
  if (!key) {
    key = { time: frame, controls: {} };
    keys.push(key);
    keys.sort((a, b) => a.time - b.time);
  }
  if (value !== undefined) key.controls[name] = Number(value);
  return key;
}

export function setEffectIntensityAt(effect, time, value) {
  return setEffectValueAt(effect, time, 'intensity', value);
}

export function createSineWaveDisplacementMap(width, height, wavelength = 80, phase = 0, direction = 'horizontal') {
  const mapWidth = 192;
  const mapHeight = Math.max(16, Math.min(256, Math.round(mapWidth * height / Math.max(1, width))));
  const canvas = document.createElement('canvas');
  canvas.width = mapWidth;
  canvas.height = mapHeight;
  const context = canvas.getContext('2d');
  const image = context.createImageData(mapWidth, mapHeight);
  const unit = Math.max(1, Math.min(width, height));
  for (let y = 0; y < mapHeight; y++) for (let x = 0; x < mapWidth; x++) {
    const px = x / (mapWidth - 1) * width;
    const py = y / (mapHeight - 1) * height;
    let dx = 0, dy = 0;
    const phasePosition = direction === 'vertical' ? px : py;
    const displacement = Math.sin(phasePosition * Math.PI * 2 / Math.max(1, wavelength) + phase) * unit * .05;
    if (direction === 'vertical') dy = displacement;
    else dx = displacement;
    const offset = (y * mapWidth + x) * 4;
    image.data[offset] = Math.max(0, Math.min(255, 128 + dx / unit * 255));
    image.data[offset + 1] = Math.max(0, Math.min(255, 128 + dy / unit * 255));
    image.data[offset + 2] = 128;
    image.data[offset + 3] = 255;
  }
  context.putImageData(image, 0, 0);
  return canvas.toDataURL('image/png');
}

