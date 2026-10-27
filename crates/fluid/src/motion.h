#pragma once

#include <QAbstractAnimation>
#include <QObject>
#include <QQmlParserStatus>
#include <QtQmlIntegration>

// One scalar spring driven by Qt's shared animation clock. There is no idle timer.
// Retargeting changes the equilibrium without resetting position or momentum.
class Motion : public QObject, public QQmlParserStatus {
    Q_OBJECT
    QML_ELEMENT
    Q_INTERFACES(QQmlParserStatus)
    Q_PROPERTY(qreal value READ value NOTIFY valueChanged)
    Q_PROPERTY(qreal target READ target WRITE setTarget NOTIFY targetChanged)
    Q_PROPERTY(qreal velocity READ velocity NOTIFY valueChanged)
    Q_PROPERTY(bool enabled READ enabled WRITE setEnabled NOTIFY enabledChanged)
    Q_PROPERTY(bool running READ running NOTIFY runningChanged)
    Q_PROPERTY(qreal stiffness READ stiffness WRITE setStiffness NOTIFY physicsChanged)
    Q_PROPERTY(qreal damping READ damping WRITE setDamping NOTIFY physicsChanged)
    Q_PROPERTY(qreal mass READ mass WRITE setMass NOTIFY physicsChanged)
    Q_PROPERTY(qreal epsilon READ epsilon WRITE setEpsilon NOTIFY physicsChanged)
public:
    explicit Motion(QObject *parent = nullptr);
    void classBegin() override {}
    void componentComplete() override;
    qreal value() const { return position; }
    qreal target() const { return goal; }
    qreal velocity() const { return speed; }
    bool enabled() const { return motionEnabled; }
    bool running() const { return clock.state() == QAbstractAnimation::Running; }
    qreal stiffness() const { return spring; }
    qreal damping() const { return drag; }
    qreal mass() const { return weight; }
    qreal epsilon() const { return tolerance; }
    void setTarget(qreal value);
    void setEnabled(bool value);
    void setStiffness(qreal value);
    void setDamping(qreal value);
    void setMass(qreal value);
    void setEpsilon(qreal value);
    Q_INVOKABLE void snap(qreal value);
    Q_INVOKABLE void resume();
signals:
    void valueChanged();
    void targetChanged();
    void enabledChanged();
    void runningChanged();
    void physicsChanged();
private:
    class Clock final : public QAbstractAnimation {
    public:
        explicit Clock(Motion *owner) : QAbstractAnimation(owner), owner(owner) {}
        int duration() const override { return -1; }
    protected:
        void updateCurrentTime(int time) override;
    private:
        Motion *owner;
    } clock;
    void step(qreal seconds);
    void settle();
    qreal position = 0, goal = 0, speed = 0;
    qreal spring = 420, drag = 36, weight = 1, tolerance = .25;
    int previousTime = 0;
    bool complete = false, motionEnabled = true;
};
