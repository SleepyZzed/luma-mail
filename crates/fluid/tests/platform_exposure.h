#pragma once
#include <QCoreApplication>
#include <QExposeEvent>
#include <QWindow>
#include <private/qwindow_p.h>

// Test-only compositor event injection. An empty damage region is not enough:
// the offscreen/X11 platform can still report the mapped window as exposed.
// Feed the same explicit exposure flag that a Wayland platform event carries.
inline void setTestExposure(QWindow &window, bool exposed) {
    QWindowPrivate::get(&window)->exposed = exposed;
    QExposeEvent event(exposed ? QRegion(QRect(QPoint(), window.size())) : QRegion());
    QCoreApplication::sendEvent(&window, &event);
    QCoreApplication::processEvents();
}
