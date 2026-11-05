#include "dango.h"
#include <math.h>
#include <string.h>

void project_init(Project *p) {
  memset(p, 0, sizeof(*p)); p->next_id = 1; p->pixels_per_second = 36.0; p->visible_video_tracks = 1;
  p->visible_audio_tracks = 1; p->snap_enabled = true; p->tool = TOOL_SELECT;
  p->controls.size = p->controls.speed = p->controls.opacity = 100.0;
  project_add_clip(p, LANE_VIDEO, 0, "coastline-morning.mp4", "", 12.0, 0.0);
  project_add_clip(p, LANE_VIDEO, 0, "market-stroll.mp4", "", 8.0, 12.0);
  project_add_clip(p, LANE_AUDIO, 0, "slow-sunday.wav", "", 26.0, 0.0);
  Clip *first = &p->clips[0]; first->keyframes[0] = (Keyframe){1.8, 100.0}; first->keyframes[1] = (Keyframe){7.0, 116.0}; first->keyframe_count = 2;
}
Clip *project_find_clip(Project *p, uint32_t id) { for (size_t i=0;i<p->clip_count;i++) if (p->clips[i].id==id) return &p->clips[i]; return NULL; }
uint32_t project_add_clip(Project *p, LaneKind kind, int track, const char *name, const char *path, double duration, double start) {
  if (!p || p->clip_count >= DANGO_MAX_CLIPS || track < 0 || track > 2) return 0;
  Clip *c=&p->clips[p->clip_count++]; memset(c,0,sizeof(*c)); c->id=p->next_id++; c->kind=kind; c->track=track;
  c->start=fmax(0.0,start); c->duration=fmax(0.25,duration); if(name) strncpy_s(c->name,sizeof(c->name),name,_TRUNCATE);
  if(path) strncpy_s(c->path,sizeof(c->path),path,_TRUNCATE);
  if(kind==LANE_VIDEO && track+1>p->visible_video_tracks) p->visible_video_tracks=track+1;
  if(kind==LANE_AUDIO && track+1>p->visible_audio_tracks) p->visible_audio_tracks=track+1;
  return c->id;
}
bool project_remove_clip(Project *p, uint32_t id) {
  if(!p)return false; size_t i=0; while(i<p->clip_count && p->clips[i].id!=id)i++; if(i==p->clip_count)return false;
  LaneKind kind=p->clips[i].kind; int track=p->clips[i].track; double start=p->clips[i].start, duration=p->clips[i].duration;
  memmove(&p->clips[i],&p->clips[i+1],(p->clip_count-i-1)*sizeof(Clip)); p->clip_count--;
  for(size_t j=0;j<p->clip_count;) { if(p->clips[j].parent_id==id){ memmove(&p->clips[j],&p->clips[j+1],(p->clip_count-j-1)*sizeof(Clip));p->clip_count--; }else j++; }
  if(p->selected_id==id)p->selected_id=0;
  if(kind==LANE_VIDEO||kind==LANE_AUDIO)for(size_t j=0;j<p->clip_count;j++)if(p->clips[j].kind==kind&&p->clips[j].track==track&&p->clips[j].start>=start+duration-0.0001)p->clips[j].start=fmax(start,p->clips[j].start-duration);
  return true;
}
void project_select_clip(Project *p,uint32_t id){if(!p)return;p->selected_id=id;for(size_t i=0;i<p->clip_count;i++)p->clips[i].selected=(p->clips[i].id==id);}
uint32_t project_add_effect(Project *p,uint32_t clip_id,const char *name){Clip *parent=project_find_clip(p,clip_id);if(!parent||parent->kind!=LANE_VIDEO)return 0;uint32_t id=project_add_clip(p,LANE_EFFECT,parent->track,name,"",parent->duration,parent->start);Clip *e=project_find_clip(p,id);if(e)e->parent_id=clip_id;return id;}
bool project_add_keyframe(Project *p,uint32_t id,double time,double value){Clip *c=project_find_clip(p,id);if(!c||c->keyframe_count>=DANGO_MAX_KEYFRAMES||time<0||time>c->duration)return false;unsigned i=c->keyframe_count++;c->keyframes[i]=(Keyframe){time,value};while(i&&c->keyframes[i-1].time>c->keyframes[i].time){Keyframe t=c->keyframes[i-1];c->keyframes[i-1]=c->keyframes[i];c->keyframes[i]=t;i--;}return true;}
void project_reset_control(Project *p,Property prop){if(!p)return;switch(prop){case PROP_POSITION:p->controls.position_x=0;p->controls.position_y=0;break;case PROP_SIZE:p->controls.size=100;break;case PROP_ROTATION:p->controls.rotation=0;break;case PROP_SPEED:p->controls.speed=100;break;case PROP_OPACITY:p->controls.opacity=100;break;default:break;}}
void project_compact_lane(Project *p,LaneKind kind,int track){if(!p)return;Clip* ordered[DANGO_MAX_CLIPS];size_t count=0;for(size_t i=0;i<p->clip_count;i++)if(p->clips[i].kind==kind&&p->clips[i].track==track)ordered[count++]=&p->clips[i];for(size_t i=1;i<count;i++){Clip* item=ordered[i];size_t j=i;while(j&&ordered[j-1]->start>item->start){ordered[j]=ordered[j-1];j--;}ordered[j]=item;}double cursor=0;for(size_t i=0;i<count;i++){Clip*c=ordered[i];c->start=cursor;cursor+=c->duration;for(size_t j=0;j<p->clip_count;j++)if(p->clips[j].kind==LANE_EFFECT&&p->clips[j].parent_id==c->id){p->clips[j].start=c->start;p->clips[j].duration=c->duration;}}}
