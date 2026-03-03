======================================
YaWC - Yet another Wayland Compositor
======================================

YaWC is a personal manual wayland compositor spread amungst a group of friends trying to make a desktop which fits their needs.

Building
=========

Prerequisites
--------------

This project will require the following packages to build:

* pipewire
* seatd
* libdisplay-info
* alsa-lib
* jack2
* udev
* pixman
* libxkbcommon
* libinput
* libgbm

These can also be fetched by entering a nix development environment using:

.. code-block:: sh
   nix develop

Running build system
---------------------

You can after you have build prerequisites run:

.. code-block:: sh
   cargo build --release

which could also be shorthanned to:

.. code-block:: sh
   cargo b -r

Runtime
========

This segment includes various different things

Dependencies
-------------

There are certain dependencies you may need to install for some things to work which have respected
features enabled.

* fontconfig
* udev
* OpenGL
* x11 (for xwayland)
* libxkbcommon

Envrionment Variables
----------------------

There are certain ENV varibles which can be used by and for YaWC.

What we look for
^^^^^^^^^^^^^^^^^

============================= ======================================= ===================
Variable                       Use Case                               Reference
============================= ======================================= ===================
YAWC_NO_VULKAN                 Skips Vulkan for X11                   ./src/x11.rs
YAWC_DISABLE_10BIT             Skips 10-bit Color for udev rendering  ./src/udev.rs
YAWC_DRM_DEVICE                Specify DRI card e.g /dev/dri/card1    ./src/udev.rs
YAWC_GLES_DISABLE_INSTANCING   Disables gles instancing               ./src/udev.rs
YAWC_DISABLE_DIRECT_SCANOUT    Disables Direct Scanout                ./src/udev.rs
============================= ======================================= ===================

What is set by us
^^^^^^^^^^^^^^^^^^

These only apply when running in a udev instance, so all can be found in ./src/udev.rs.

============================= ===================== ==========================================================================================
Variable                       Value                Use Case                              
============================= ===================== ==========================================================================================
XDG_CURRENT_DESKTOP            YaWC                 Identify that it is us as the desktop
XDG_SESSION_DESKTOP            YaWC                 Identify that it is us as the desktop
XDG_SESSION_TYPE               wayland              Identify that it is a Wayland instance
QT_QPA_PLATFORM                wayland;xcb          Get QT apps to attempt to load QT's wayland plugin and if not; failover to xcb plugin
ELECTRON_OZONE_PLATFORM_HINT   wayland              Hint to Electron that our session is Wayland.
GTK_BACKEND                    wayland              For GTK3.0-or-later to know we are wayland (MOZ_ENABLE_WAYLAND is not needed due to this)
============================= ===================== ==========================================================================================

Licensing
==========

The start of this project was created with the help of Anvil, the example compositor which Smithay uses to test, which was MIT.

The project state as it stands today is under AGPL-v3.0.