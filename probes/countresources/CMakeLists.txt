cmake_minimum_required(VERSION 3.16)
project(CountResourcesProbe C)
add_application(CountResourcesProbe count.c fixtures.r ../saneround/memory.r)
