#include "motion.h"
#include <algorithm>
#include <cmath>

Motion::Motion(QObject *parent) : QObject(parent), clock(this) {
    connect(&clock, &QAbstractAnimation::stateChanged, this, [this] { emit runningChanged(); });
}

void Motion::componentComplete() {
    complete = true;
    settle();
}

void Motion::setTarget(qreal value) {
    if (!std::isfinite(value) || value == goal) return;
    goal = value;
    emit targetChanged();
    if (!complete || !motionEnabled) {
        settle();
    } else resume();
}

void Motion::resume() {
    if (!complete || !motionEnabled) { settle(); return; }
    if (!running() && (position != goal || speed != 0)) {
        previousTime = 0;
        clock.start();
    }
}

void Motion::setEnabled(bool value) {
    if (motionEnabled == value) return;
    motionEnabled = value;
    emit enabledChanged();
    if (!value) settle();
}

void Motion::setStiffness(qreal value) {
    if (!std::isfinite(value)) return;
    spring = std::clamp(value, qreal(1), qreal(2000));
    emit physicsChanged();
}
void Motion::setDamping(qreal value) {
    if (!std::isfinite(value)) return;
    drag = std::clamp(value, qreal(.1), qreal(200));
    emit physicsChanged();
}
void Motion::setMass(qreal value) {
    if (!std::isfinite(value)) return;
    weight = std::clamp(value, qreal(.1), qreal(10));
    emit physicsChanged();
}
void Motion::setEpsilon(qreal value) {
    if (!std::isfinite(value)) return;
    tolerance = std::clamp(value, qreal(.0001), qreal(2));
    emit physicsChanged();
}

void Motion::snap(qreal value) {
    if (!std::isfinite(value)) return;
    clock.stop();
    position = value;
    speed = 0;
    emit valueChanged();
}
void Motion::rebase(qreal value){
    if(!std::isfinite(value))return;
    position=value;emit valueChanged();resume();
}

void Motion::settle() {
    clock.stop();
    const bool changed = position != goal || speed != 0;
    position = goal;
    speed = 0;
    if (changed) emit valueChanged();
}

void Motion::Clock::updateCurrentTime(int time) {
    const int elapsed = time - owner->previousTime;
    owner->previousTime = time;
    if (elapsed > 0) owner->step(elapsed / 1000.0);
}

void Motion::step(qreal seconds) {
    // Exact damped harmonic solution at the actual animation-clock interval.
    // Unlike a fixed 16 ms integration step this follows high-refresh displays
    // and remains stable after a delayed frame or when retargeted repeatedly.
    const qreal displacement = position - goal;
    const qreal alpha = drag / (2 * weight);
    const qreal omegaSquared = spring / weight;
    const qreal discriminant = alpha * alpha - omegaSquared;
    if (std::abs(discriminant) < .000001) {
        const qreal b = speed + alpha * displacement;
        const qreal decay = std::exp(-alpha * seconds);
        position = goal + (displacement + b * seconds) * decay;
        speed = (b - alpha * (displacement + b * seconds)) * decay;
    } else if (discriminant < 0) {
        const qreal omega = std::sqrt(-discriminant);
        const qreal b = (speed + alpha * displacement) / omega;
        const qreal angle = omega * seconds;
        const qreal decay = std::exp(-alpha * seconds);
        const qreal oscillation = displacement * std::cos(angle) + b * std::sin(angle);
        position = goal + decay * oscillation;
        speed = decay * ((-displacement * omega * std::sin(angle) + b * omega * std::cos(angle)) - alpha * oscillation);
    } else {
        const qreal root = std::sqrt(discriminant);
        const qreal first = -alpha + root, second = -alpha - root;
        const qreal a = (speed - second * displacement) / (first - second);
        const qreal b = displacement - a;
        const qreal firstTerm = a * std::exp(first * seconds);
        const qreal secondTerm = b * std::exp(second * seconds);
        position = goal + firstTerm + secondTerm;
        speed = first * firstTerm + second * secondTerm;
    }
    emit valueChanged();
    if (std::abs(position - goal) < tolerance && std::abs(speed) < tolerance * 8) settle();
}
