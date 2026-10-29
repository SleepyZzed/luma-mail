#include "window_activity.h"
#include <QEvent>
#include <QTimer>

WindowActivity *WindowActivity::forWindow(QWindow *window) {
    if (!window) return nullptr;
    if (auto existing = window->findChild<WindowActivity *>(QString(), Qt::FindDirectChildrenOnly)) return existing;
    return new WindowActivity(window);
}

WindowActivity::WindowActivity(QWindow *window) : QObject(window), host(window) {
    window->installEventFilter(this);
    connect(window, &QWindow::visibleChanged, this, &WindowActivity::update);
    connect(window, &QWindow::visibilityChanged, this, &WindowActivity::update);
    update();
}

void WindowActivity::update() {
    const bool next = host && host->isVisible() && host->visibility() != QWindow::Minimized && host->isExposed();
    if (next == rendering) return;
    rendering = next;
    emit renderableChanged();
}

bool WindowActivity::eventFilter(QObject *object, QEvent *event) {
    if (object == host && (event->type() == QEvent::Expose || event->type() == QEvent::PlatformSurface)) {
        // Exposure is owned by Qt's platform event delivery. Coalesce the final
        // state without polling or requesting a new frame on a hidden surface.
        if (!updateQueued) {
            updateQueued = true;
            QTimer::singleShot(0, this, [this] { updateQueued = false; update(); });
        }
    }
    return QObject::eventFilter(object, event);
}
