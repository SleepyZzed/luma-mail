#include "motion.h"
#include <QSignalSpy>
#include <QtTest>
#include <limits>
#include <cmath>

class MotionTest : public QObject {
    Q_OBJECT
private slots:
    void initialLayoutDoesNotAnimate() {
        Motion motion;
        motion.setTarget(500);
        motion.componentComplete();
        QCOMPARE(motion.value(), 500);
        QVERIFY(!motion.running());
    }
    void interruptionPreservesPositionAndMomentum() {
        Motion motion;
        motion.componentComplete();
        motion.setTarget(900);
        QTRY_VERIFY_WITH_TIMEOUT(motion.value() > 100, 500);
        const auto position = motion.value(), velocity = motion.velocity();
        QVERIFY(velocity > 0);
        motion.setTarget(-200);
        QCOMPARE(motion.value(), position);
        QCOMPARE(motion.velocity(), velocity);
        QVERIFY(motion.running());
        QTRY_VERIFY_WITH_TIMEOUT(!motion.running(), 2500);
        QCOMPARE(motion.value(), -200);
        QCOMPARE(motion.velocity(), 0);
    }
    void reducedMotionStopsImmediately() {
        Motion motion;
        motion.componentComplete();
        motion.setTarget(600);
        QTRY_VERIFY(motion.value() > 0);
        motion.setEnabled(false);
        QCOMPARE(motion.value(), 600);
        QVERIFY(!motion.running());
        motion.setTarget(15);
        QCOMPARE(motion.value(), 15);
        QVERIFY(!motion.running());
    }
    void reparentingCoordinatesKeepTheExistingMomentum(){
        Motion motion;motion.componentComplete();motion.setTarget(900);QTRY_VERIFY_WITH_TIMEOUT(motion.value()>50,500);
        const auto speed=motion.velocity(),position=motion.value();motion.rebase(position+200);
        QCOMPARE(motion.value(),position+200);QCOMPARE(motion.velocity(),speed);QVERIFY(motion.running());
        QTRY_VERIFY_WITH_TIMEOUT(!motion.running(),2500);QCOMPARE(motion.value(),900);
    }
    void settledSpringDoesNoFurtherWork() {
        Motion motion;
        motion.componentComplete();
        motion.setTarget(10);
        QTRY_VERIFY_WITH_TIMEOUT(!motion.running(), 2500);
        QSignalSpy changes(&motion, &Motion::valueChanged);
        QTest::qWait(120);
        QCOMPARE(changes.count(), 0);
    }
    void repeatedTargetsStayFinite() {
        Motion motion;
        motion.componentComplete();
        for (int i = 0; i < 20; ++i) {
            motion.setTarget(i % 2 ? -600 : 900);
            QTest::qWait(11);
            QVERIFY(std::isfinite(motion.value()));
        }
        motion.setTarget(std::numeric_limits<qreal>::quiet_NaN());
        QCOMPARE(motion.target(), -600);
        QTRY_VERIFY_WITH_TIMEOUT(!motion.running(), 2500);
        QCOMPARE(motion.value(), -600);
    }
};
QTEST_GUILESS_MAIN(MotionTest)
#include "motion_test.moc"
