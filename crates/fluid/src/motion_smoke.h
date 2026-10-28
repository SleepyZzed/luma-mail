#pragma once
#include <QElapsedTimer>
#include <QJsonArray>
#include <QObject>
#include <QPointer>
#include <functional>
class Bridge;
class QQuickItem;
class QQuickWindow;

// Runs only with --design-preview --motion-test and synthetic engine data.
class MotionSmoke : public QObject {
public:
    MotionSmoke(Bridge *bridge, QQuickWindow *window);
private:
    void begin();
    void check(const QString &name, bool passed);
    void open(int index, int modifiers=0);
    void finish();
    void whenStill(QQuickItem*item,std::function<void()> next,int attempts=80);
    Bridge *bridge;
    QQuickWindow *window;
    QQuickItem *navigation=nullptr, *inbox=nullptr, *morph=nullptr;
    QJsonArray checks;
    QElapsedTimer elapsed;
    QList<qint64> frames;
    qint64 totalFrames=0;
    bool removalAnimated=false;
    bool failed=false, started=false, finished=false;
};
