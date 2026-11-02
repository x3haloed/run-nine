cmake_minimum_required(VERSION 3.16)
project(WindowResourcesProbe C)
add_application(WindowResourcesProbe windows.c fixtures.r)
add_application(WindowResourceFlowProbe lifecycle.c fixtures.r)
