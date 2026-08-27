# Copyright Amazon.com Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0 OR ISC

# GenerateVersionScript.cmake
#
# Provides apply_version_script() to link a GNU ld version script into an ELF
# shared library target. Version scripts are maintained as checked-in source
# files (crypto/libcrypto.map, ssl/libssl.map) and generated from the symbol
# registries (crypto/libcrypto.txt, ssl/libssl.txt) via:
#
#   util/generate_initial_version_scripts.sh   (initial population)
#   util/update_symbol_version.sh <version>    (adding new symbols)

# apply_version_script()
#
# Applies a GNU ld version script to a library target for symbol versioning.
#
# Parameters:
#   TARGET         - Library target name (e.g., crypto, ssl)
#   VERSION_SCRIPT - Path to version script file (e.g., ${CMAKE_CURRENT_SOURCE_DIR}/libcrypto.map)

function(apply_version_script)
  set(options "")
  set(oneValueArgs TARGET VERSION_SCRIPT)
  set(multiValueArgs "")
  cmake_parse_arguments(PARSE_ARGV 0 ARG "${options}" "${oneValueArgs}" "${multiValueArgs}")

  if(NOT DEFINED ARG_TARGET)
    message(FATAL_ERROR "apply_version_script: TARGET is required")
  endif()

  if(NOT DEFINED ARG_VERSION_SCRIPT)
    message(FATAL_ERROR "apply_version_script: VERSION_SCRIPT is required")
  endif()

  if(NOT UNIX OR APPLE)
    message(STATUS "Symbol versioning not supported on this platform (requires GNU ld or compatible)")
    return()
  endif()

  if(NOT EXISTS "${ARG_VERSION_SCRIPT}")
    message(FATAL_ERROR "apply_version_script: Version script not found: ${ARG_VERSION_SCRIPT}")
  endif()

  target_link_options(${ARG_TARGET} PRIVATE
    "LINKER:--version-script=${ARG_VERSION_SCRIPT}"
  )

  # The version script is passed via a linker flag, which CMake does not parse
  # to discover a build dependency. Record it explicitly so editing the .map
  # (e.g. regenerating it from the registry) triggers a re-link.
  set_target_properties(${ARG_TARGET} PROPERTIES
    LINK_DEPENDS "${ARG_VERSION_SCRIPT}"
  )

  if(LINKER_HAS_UNDEFINED_VERSION)
    target_link_options(${ARG_TARGET} PRIVATE
      "LINKER:--undefined-version"
    )
  endif()

  get_filename_component(VERSION_SCRIPT_NAME "${ARG_VERSION_SCRIPT}" NAME)
  message(STATUS "Applied symbol version script to ${ARG_TARGET}: ${VERSION_SCRIPT_NAME}")
endfunction()
