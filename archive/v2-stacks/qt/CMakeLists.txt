cmake_minimum_required(VERSION 3.25)   # Qt 6.12 requires CMake >= 3.25 (QT_SUPPORTED_MIN_CMAKE_VERSION_FOR_USING_QT)
project(qtbench VERSION 2.0 LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

# Repository root: BENCH_ROOT (env or -D), otherwise two levels above this file. No absolute path hard-coded.
if(NOT BENCH_ROOT)
  if(DEFINED ENV{BENCH_ROOT})
    set(BENCH_ROOT "$ENV{BENCH_ROOT}")
  else()
    get_filename_component(BENCH_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
  endif()
endif()
set(CORE_DIR "${BENCH_ROOT}/core")
if(NOT EXISTS "${CORE_DIR}/target/release/libbenchcore.a")
  message(FATAL_ERROR "libbenchcore.a not found in ${CORE_DIR}/target/release (build core/ first)")
endif()

find_package(Threads REQUIRED)
find_package(Qt6 6.12 REQUIRED COMPONENTS Core Gui Quick QuickControls2)
qt_standard_project_setup(REQUIRES 6.12)   # AUTOMOC, GNUInstallDirs, QML module policies of Qt 6.12

qt_add_executable(qtbench main.cpp)
qt_add_qml_module(qtbench
  URI Bench
  VERSION 1.0
  QML_FILES qml/Main.qml qml/Shell.qml qml/ListPane.qml qml/TextPane.qml qml/ImagesPane.qml
            qml/CanvasPane.qml qml/CanvasJs.qml qml/NavPane.qml qml/NavScreen.qml
  SOURCES bench.h model.h canvas.h common.h
)
target_include_directories(qtbench PRIVATE "${CORE_DIR}")
# Rust C-ABI core linked statically (spec: static or dynamic at the stack's choice).
target_link_libraries(qtbench PRIVATE Qt6::Quick "${CORE_DIR}/target/release/libbenchcore.a" Threads::Threads ${CMAKE_DL_LIBS} m)

# Official Qt 6 deployment (qt_generate_deploy_qml_app_script): `cmake --install build --prefix deploy`
install(TARGETS qtbench BUNDLE DESTINATION . RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
qt_generate_deploy_qml_app_script(TARGET qtbench OUTPUT_SCRIPT deploy_script NO_TRANSLATIONS)
install(SCRIPT ${deploy_script})
# libxcb-cursor.so.0 is required by Qt's xcb platform plugin since 6.5 and is not installed on this machine (no apt):
# bundled from Ubuntu noble's libxcb-cursor0 0.1.4-1build1 (fetched + sha256-checked by build.sh) next to the Qt libs.
install(FILES thirdparty/libxcb-cursor.so.0 thirdparty/libxcb-cursor.so.0.0.0 DESTINATION ${CMAKE_INSTALL_LIBDIR})
