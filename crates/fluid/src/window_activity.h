#pragma once
#include <QObject>
#include <QPointer>
#include <QWindow>

// A mapped window is not necessarily being presented. Wayland compositors stop
// exposing surfaces on other workspaces without hiding or minimizing QWindow.
// One event-driven observer is shared by every consumer of a native window.
class WindowActivity final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool renderable READ renderable NOTIFY renderableChanged)
public:
    static WindowActivity *forWindow(QWindow *window);
    bool renderable() const { return rendering; }
signals:
    void renderableChanged();
protected:
    bool eventFilter(QObject *object, QEvent *event) override;
private:
    explicit WindowActivity(QWindow *window);
    void update();
    QPointer<QWindow> host;
    bool rendering = false, updateQueued = false;
};
