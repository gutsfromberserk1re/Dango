#ifndef DANGO_H
#define DANGO_H

#include <windows.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define DANGO_MAX_CLIPS 256
#define DANGO_MAX_KEYFRAMES 32
#define DANGO_MAX_PATH 1024

typedef enum { LANE_VIDEO = 0, LANE_AUDIO = 1, LANE_EFFECT = 2 } LaneKind;
typedef enum { TOOL_SELECT, TOOL_BLADE, TOOL_HAND, TOOL_TEXT } Tool;
typedef enum { TAB_KONTROL, TAB_EFFECTZ, TAB_MEDIA } InspectorTab;
typedef enum { PROP_POSITION, PROP_SIZE, PROP_ROTATION, PROP_SPEED, PROP_OPACITY, PROP_COUNT } Property;
typedef struct { double time; double value; } Keyframe;
typedef struct {
  uint32_t id;
  LaneKind kind;
  int track;
  double start, duration;
  char name[192];
  char path[DANGO_MAX_PATH];
  unsigned keyframe_count;
  Keyframe keyframes[DANGO_MAX_KEYFRAMES];
  uint32_t parent_id; /* Effects refer to their affected clip. */
  bool selected;
} Clip;
typedef struct { double position_x, position_y, size, rotation, speed, opacity; } ClipControls;
typedef struct {
  Clip clips[DANGO_MAX_CLIPS];
  size_t clip_count;
  uint32_t next_id;
  uint32_t selected_id;
  ClipControls controls;
  double playhead;
  double pixels_per_second;
  double scroll_seconds;
  int visible_video_tracks, visible_audio_tracks;
  bool snap_enabled;
  bool playing;
  Tool tool;
} Project;
typedef struct {
  HWND hwnd;
  Project project;
  InspectorTab tab;
  RECT preview, inspector, timeline, toolrail;
  int splitter_left, splitter_center, splitter_bottom;
  int drag_splitter;
  int drag_clip;
  POINT drag_origin;
  double drag_start;
  bool dragging_clip;
  int window_width, window_height;
  double panel_left_ratio, inspector_ratio, timeline_ratio;
  wchar_t status[256];
} DangoApp;

void project_init(Project *project);
Clip *project_find_clip(Project *project, uint32_t id);
uint32_t project_add_clip(Project *project, LaneKind kind, int track, const char *name, const char *path, double duration, double start);
bool project_remove_clip(Project *project, uint32_t id);
void project_select_clip(Project *project, uint32_t id);
uint32_t project_add_effect(Project *project, uint32_t clip_id, const char *name);
bool project_add_keyframe(Project *project, uint32_t clip_id, double time, double value);
void project_reset_control(Project *project, Property property);
void project_compact_lane(Project *project, LaneKind kind, int track);
void ui_paint(DangoApp *app, HDC hdc);
void ui_layout(DangoApp *app, int width, int height);
void ui_mouse_down(DangoApp *app, int x, int y, UINT flags);
void ui_mouse_move(DangoApp *app, int x, int y, UINT flags);
void ui_mouse_up(DangoApp *app, int x, int y);
void ui_key(DangoApp *app, WPARAM key);
void media_import_dialog(DangoApp *app);
bool media_probe(const char *path, double *duration, char *error, size_t error_size);

#endif
