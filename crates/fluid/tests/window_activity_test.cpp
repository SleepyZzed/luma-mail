#include "window_activity.h"
#include <QtTest>
#include <QSignalSpy>
#include "platform_exposure.h"

class WindowActivityTest : public QObject {
    Q_OBJECT
    static void expose(QWindow &window, bool value) {
        setTestExposure(window, value);
    }
private slots:
    void hiddenWindowsStartInactive() {
        QWindow window;
        auto activity = WindowActivity::forWindow(&window);
        QVERIFY(!activity->renderable());
        QCOMPARE(WindowActivity::forWindow(&window), activity);
        QCOMPARE(window.findChildren<WindowActivity *>().size(), 1);
        QVERIFY(!WindowActivity::forWindow(nullptr));
    }
    void compositorExposureIsIndependentOfMappedVisibility() {
        QWindow window;
        window.resize(320, 240);
        auto activity = WindowActivity::forWindow(&window);
        QSignalSpy changed(activity, &WindowActivity::renderableChanged);
        window.show();
        QVERIFY(QTest::qWaitForWindowExposed(&window));
        QTRY_VERIFY(activity->renderable());
        for (int cycle = 0; cycle < 20; ++cycle) {
            expose(window, false);
            QVERIFY(window.isVisible());
            QVERIFY(window.visibility() != QWindow::Minimized);
            QVERIFY(!window.isExposed());
            QTRY_VERIFY(!activity->renderable());
            expose(window, true);
            QTRY_VERIFY(activity->renderable());
        }
        QVERIFY(changed.count() >= 40);
        const auto before = changed.count();
        QTest::qWait(120);
        QCOMPARE(changed.count(), before); // no idle timer/repaint loop
        window.hide();
        QTRY_VERIFY(!activity->renderable());
    }
    void windowsHaveIndependentActivity() {
        QWindow first, second;
        first.resize(320, 240);second.resize(320, 240);
        auto main = WindowActivity::forWindow(&first);
        auto composer = WindowActivity::forWindow(&second);
        first.show();second.show();
        QVERIFY(QTest::qWaitForWindowExposed(&first));
        QVERIFY(QTest::qWaitForWindowExposed(&second));
        QTRY_VERIFY(main->renderable() && composer->renderable());
        expose(first, false);
        QTRY_VERIFY(!main->renderable());
        QVERIFY(composer->renderable());
        expose(second, false);
        QTRY_VERIFY(!composer->renderable());
        expose(first, true);
        QTRY_VERIFY(main->renderable());
        QVERIFY(!composer->renderable());
    }
};
QTEST_MAIN(WindowActivityTest)
#include "window_activity_test.moc"
