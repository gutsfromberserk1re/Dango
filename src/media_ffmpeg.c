#include "dango.h"
#include <libavformat/avformat.h>
#include <stdio.h>

bool media_probe(const char *path,double *duration,char *error,size_t error_size){
  if(!path||!duration)return false;AVFormatContext *format=NULL;int result=avformat_open_input(&format,path,NULL,NULL);
  if(result<0){if(error&&error_size)snprintf(error,error_size,"FFmpeg could not open this media file (error %d).",result);return false;}
  result=avformat_find_stream_info(format,NULL);if(result<0){if(error&&error_size)snprintf(error,error_size,"FFmpeg could not read the media streams (error %d).",result);avformat_close_input(&format);return false;}
  *duration=format->duration>0?(double)format->duration/AV_TIME_BASE:1.0;avformat_close_input(&format);return true;
}
