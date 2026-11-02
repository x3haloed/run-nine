cmake_minimum_required(VERSION 3.16)
project(ModalFilterProbe C)
add_application(ModalFilterProbe filter.c dialog.r)

# Small independent 68K callback fixtures for the shared-dispatch runtime probe.
foreach(name hook filter useritem)
  add_custom_command(OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/runtime-${name}.bin
    COMMAND ${CMAKE_C_COMPILER} -c ${CMAKE_CURRENT_SOURCE_DIR}/runtime-${name}.s -o ${CMAKE_CURRENT_BINARY_DIR}/runtime-${name}.o
    COMMAND ${CMAKE_OBJCOPY} -O binary --only-section=.text ${CMAKE_CURRENT_BINARY_DIR}/runtime-${name}.o ${CMAKE_CURRENT_BINARY_DIR}/runtime-${name}.bin
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/runtime-${name}.s)
  add_custom_target(runtime_${name} ALL DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/runtime-${name}.bin)
endforeach()
add_application(StandardFilterProbe standard.c dialog.r)

add_application(UserItemsProbe useritems.c useritems.r)

add_application(IconPlotProbe iconplot.c iconplot.r)
