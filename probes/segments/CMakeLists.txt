cmake_minimum_required(VERSION 3.16)
project(SegmentProbe C)
add_application(SegmentProbe segments.c)
set_target_properties(SegmentProbe PROPERTIES LINK_FLAGS "-Wl,-gc-sections -Wl,--mac-single")
