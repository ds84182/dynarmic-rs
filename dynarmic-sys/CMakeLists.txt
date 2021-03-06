project(dynarmic-wrapper CXX)

add_subdirectory(dynarmic)

set(CMAKE_CXX_STANDARD 17)

add_library(dynarmic_wrapper wrapper.cpp)
add_dependencies(dynarmic_wrapper dynarmic)
target_link_libraries(dynarmic_wrapper dynarmic)

install(TARGETS dynarmic_wrapper ARCHIVE DESTINATION .)

if ( MSVC )
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /std:c++17")

    set_target_properties( dynarmic PROPERTIES ARCHIVE_OUTPUT_DIRECTORY_DEBUG ${CMAKE_INSTALL_PREFIX} )
    set_target_properties( dynarmic PROPERTIES ARCHIVE_OUTPUT_DIRECTORY_RELEASE ${CMAKE_INSTALL_PREFIX} )

    set_target_properties( fmt PROPERTIES ARCHIVE_OUTPUT_DIRECTORY_DEBUG ${CMAKE_INSTALL_PREFIX} )
    set_target_properties( fmt PROPERTIES ARCHIVE_OUTPUT_DIRECTORY_RELEASE ${CMAKE_INSTALL_PREFIX} )
    # etc for the other available configuration types (MinSizeRel, RelWithDebInfo)
endif ( MSVC )
