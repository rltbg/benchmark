cmake_minimum_required(VERSION 3.25)   # Qt 6.12 : CMake >= 3.25
project(qtspike VERSION 1.0 LANGUAGES C CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE Release)
endif()

find_package(Qt6 6.12 REQUIRED COMPONENTS Core Gui Quick)
qt_standard_project_setup(REQUIRES 6.12)

# Cœur vérifié : C généré par F* Pulse -> KaRaMeL (Custard.c/.h, inchangés), compilé comme cible C de cette plate-forme.
set(CUSTARD_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../stacks/karamel/gen")
add_library(custard STATIC "${CUSTARD_DIR}/Custard.c")
target_include_directories(custard PUBLIC "${CUSTARD_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}/core")
set_target_properties(custard PROPERTIES POSITION_INDEPENDENT_CODE ON)

qt_add_executable(qtspike
  src/main.cpp src/spike.cpp src/spike.h src/prices.h src/shapes.h)
qt_add_qml_module(qtspike
  URI Spike VERSION 1.0
  QML_FILES qml/Main.qml qml/ListTab.qml qml/CanvasTab.qml qml/CoreTab.qml
  SOURCES src/spike.h src/prices.h src/shapes.h)
target_include_directories(qtspike PRIVATE src)
target_link_libraries(qtspike PRIVATE Qt6::Quick custard)

if(WIN32)
  set_target_properties(qtspike PROPERTIES WIN32_EXECUTABLE FALSE)   # console conservée : stderr = marqueurs SPIKE:
endif()
if(APPLE)
  set_target_properties(qtspike PROPERTIES MACOSX_BUNDLE TRUE
    MACOSX_BUNDLE_GUI_IDENTIFIER "org.example.qtspike"
    MACOSX_BUNDLE_BUNDLE_NAME "qtspike")
endif()

# Déploiement officiel Qt pour le bureau (cmake --install ; dossier autonome / .app) : Linux, Windows, macOS.
if(NOT ANDROID AND NOT IOS AND NOT EMSCRIPTEN)
  install(TARGETS qtspike BUNDLE DESTINATION . RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
  qt_generate_deploy_qml_app_script(TARGET qtspike OUTPUT_SCRIPT deploy_script NO_TRANSLATIONS)
  install(SCRIPT ${deploy_script})
endif()

# Android : paquet + ABI choisi par le kit Qt utilisé (arm64-v8a ou x86_64) ; cibles `apk` et `aab` fournies par Qt.
if(ANDROID)
  set_target_properties(qtspike PROPERTIES QT_ANDROID_PACKAGE_NAME "org.example.qtspike")
endif()
