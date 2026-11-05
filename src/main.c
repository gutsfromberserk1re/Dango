#include "dango.h"
#include <dwmapi.h>
#include <commdlg.h>
#include <windowsx.h>
#include <math.h>
#include <string.h>

static DangoApp app;
static void set_status(const wchar_t *text){wcsncpy_s(app.status,256,text,_TRUNCATE);InvalidateRect(app.hwnd,NULL,FALSE);}
void media_import_dialog(DangoApp *a){wchar_t file[DANGO_MAX_PATH]={0};OPENFILENAMEW ofn={0};ofn.lStructSize=sizeof(ofn);ofn.hwndOwner=a->hwnd;ofn.lpstrFile=file;ofn.nMaxFile=DANGO_MAX_PATH;ofn.lpstrFilter=L"Media files\0*.mp4;*.mov;*.mkv;*.avi;*.wav;*.mp3;*.png;*.jpg;*.jpeg\0All files\0*.*\0";ofn.Flags=OFN_FILEMUSTEXIST|OFN_PATHMUSTEXIST;
  if(!GetOpenFileNameW(&ofn))return;char path[DANGO_MAX_PATH];WideCharToMultiByte(CP_UTF8,0,file,-1,path,sizeof(path),NULL,NULL);double duration=10;char error[180];if(media_probe(path,&duration,error,sizeof(error))){char *base=strrchr(path,'\\');char *ext=strrchr(path,'.');LaneKind kind=(ext&&(_stricmp(ext,".wav")==0||_stricmp(ext,".mp3")==0||_stricmp(ext,".flac")==0))?LANE_AUDIO:LANE_VIDEO;int track=kind==LANE_AUDIO?0:0;double start=0;for(size_t i=0;i<a->project.clip_count;i++)if(a->project.clips[i].kind==kind&&a->project.clips[i].track==track)start=fmax(start,a->project.clips[i].start+a->project.clips[i].duration);project_add_clip(&a->project,kind,track,base?base+1:path,path,duration,start);set_status(L"Imported and probed by FFmpeg.");}else set_status(L"FFmpeg could not import that file.");}
static LRESULT CALLBACK wndproc(HWND hwnd,UINT msg,WPARAM wp,LPARAM lp){switch(msg){case WM_CREATE:app.hwnd=hwnd;return 0;case WM_SIZE:ui_layout(&app,LOWORD(lp),HIWORD(lp));InvalidateRect(hwnd,NULL,FALSE);return 0;case WM_PAINT:{PAINTSTRUCT ps;HDC dc=BeginPaint(hwnd,&ps);ui_paint(&app,dc);EndPaint(hwnd,&ps);return 0;}case WM_LBUTTONDOWN:SetCapture(hwnd);ui_mouse_down(&app,GET_X_LPARAM(lp),GET_Y_LPARAM(lp),(UINT)wp);return 0;case WM_MOUSEMOVE:ui_mouse_move(&app,GET_X_LPARAM(lp),GET_Y_LPARAM(lp),(UINT)wp);return 0;case WM_LBUTTONUP:ReleaseCapture();ui_mouse_up(&app,GET_X_LPARAM(lp),GET_Y_LPARAM(lp));return 0;case WM_KEYDOWN:ui_key(&app,wp);return 0;case WM_ERASEBKGND:return 1;case WM_DESTROY:PostQuitMessage(0);return 0;default:return DefWindowProcW(hwnd,msg,wp,lp);}}
int WINAPI wWinMain(HINSTANCE instance,HINSTANCE previous,PWSTR command,int show){(void)previous;(void)command;SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);project_init(&app.project);app.panel_left_ratio=.055;app.inspector_ratio=.245;app.timeline_ratio=.34;app.drag_splitter=-1;app.drag_clip=-1;app.tab=TAB_KONTROL;
  const wchar_t cls[]=L"DangoNativeWindow";WNDCLASSEXW wc={sizeof(wc)};wc.hInstance=instance;wc.lpfnWndProc=wndproc;wc.lpszClassName=cls;wc.hCursor=LoadCursor(NULL,IDC_ARROW);wc.hbrBackground=(HBRUSH)(COLOR_WINDOW+1);RegisterClassExW(&wc);
  HWND hwnd=CreateWindowExW(0,cls,L"Dango — Simple video editing",WS_OVERLAPPEDWINDOW|WS_CLIPCHILDREN,CW_USEDEFAULT,CW_USEDEFAULT,1440,900,NULL,NULL,instance,NULL);if(!hwnd)return 1;
  /* Windows 11 system backdrop; gracefully ignored by older builds. */
  /* DWM system-backdrop support landed after the SDK on some hosted runners.
     The documented attribute/value pair is 38 / DWMSBT_TRANSIENTWINDOW (3).
     Older Windows builds simply reject the attribute; the editor remains opaque. */
  const DWORD backdrop_attribute=38;const int backdrop_value=3;
  DwmSetWindowAttribute(hwnd,(DWMWINDOWATTRIBUTE)backdrop_attribute,&backdrop_value,sizeof(backdrop_value));
  ShowWindow(hwnd,show);UpdateWindow(hwnd);MSG msg;while(GetMessageW(&msg,NULL,0,0)>0){TranslateMessage(&msg);DispatchMessageW(&msg);}return (int)msg.wParam;}
