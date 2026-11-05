cmake_minimum_required(VERSION 3.24)
project(Dango VERSION 0.1.0 LANGUAGES C)

if(NOT WIN32)
  message(FATAL_ERROR "Dango's first native UI backend targets Windows.")
endif()

set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

find_path(FFMPEG_INCLUDE_DIR libavformat/avformat.h REQUIRED)
foreach(component avformat avcodec avutil avfilter swscale swresample)
  find_library(FFMPEG_${component}_LIBRARY NAMES ${component} REQUIRED)
  list(APPEND FFMPEG_LIBRARIES ${FFMPEG_${component}_LIBRARY})
endforeach()

add_executable(Dango WIN32
  src/main.c
  src/model.c
  src/ui.c
  src/media_ffmpeg.c
)
target_include_directories(Dango PRIVATE include)
target_include_directories(Dango SYSTEM PRIVATE ${FFMPEG_INCLUDE_DIR})
target_link_libraries(Dango PRIVATE ${FFMPEG_LIBRARIES} comdlg32 dwmapi shcore)
target_compile_definitions(Dango PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
if(MSVC)
  target_compile_options(Dango PRIVATE /W4 /utf-8)
else()
  target_compile_options(Dango PRIVATE -Wall -Wextra -Wpedantic)
endif()
