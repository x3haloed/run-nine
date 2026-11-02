cmake_minimum_required(VERSION 3.16)
project(TEOutlineProbe C)
add_application(TEOutlineProbe outline.c)

add_application(TEOutlineControlProbe control.c)
add_application(TEOutlineSpacesProbe spaces.c)

add_application(TEOutlineRefreshProbe refresh.c)
