#include "html_surface.h"
#include "reader_profile.h"
#include "window_activity.h"
#include <QGuiApplication>
#include <QQuickWindow>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QFile>
#include <QSignalSpy>
#include <QtTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include "platform_exposure.h"

class ReaderTheme : public QObject {
    Q_OBJECT
    Q_PROPERTY(QColor window READ window CONSTANT)
    Q_PROPERTY(QColor muted READ muted CONSTANT)
public:
    QColor window() const { return QColor("#19171e"); }
    QColor muted() const { return QColor("#aaaaaa"); }
    Q_INVOKABLE qreal unit(qreal value) const { return value; }
};
class ReaderMail : public QObject {
    Q_OBJECT
public:
    Q_INVOKABLE void openLink(const QString &) {}
};
class ReaderLifecycleTest : public QObject {
    Q_OBJECT
    static void expose(QWindow &window, bool value) {
        setTestExposure(window, value);
    }
private slots:
    void mappedWorkspaceAndModalSuspendWithoutRecreatingMail() {
        QQuickWindow window;
        window.resize(640, 480);
        QQmlEngine engine;
        ReaderTheme theme;ReaderMail mail;ReaderProfiles profiles;
        engine.rootContext()->setContextProperty("Theme", &theme);
        engine.rootContext()->setContextProperty("Mail", &mail);
        engine.rootContext()->setContextProperty("ReaderProfiles", &profiles);
        QFile source(QStringLiteral(LUMA_READER_QML));
        QVERIFY(source.open(QIODevice::ReadOnly));
        auto qml = source.readAll();
        qml.replace("import QtQuick\n", "import QtQuick\nimport Test.Fluid\n");
        QQmlComponent component(&engine);
        component.setData(qml, QUrl("inmemory:/LinuxReader.qml"));
        QTRY_VERIFY_WITH_TIMEOUT(component.status() != QQmlComponent::Loading, 10000);
        QVERIFY2(!component.isError(), qPrintable(component.errorString()));
        auto reader = qobject_cast<HtmlSurface *>(component.beginCreate(engine.rootContext()));
        QVERIFY2(reader, qPrintable(component.errorString()));
        reader->setParentItem(window.contentItem());
        reader->setSize(QSizeF(600, 1200));
        reader->setProperty("page", "<html><body style='height:1100px;background:#19171e;color:white'><table><tr><td>Workspace regression fixture</td></tr></table></body></html>");
        component.completeCreate();
        QVERIFY2(!component.isError(), qPrintable(component.errorString()));
        window.show();
        QVERIFY(QTest::qWaitForWindowExposed(&window));
        QTRY_VERIFY_WITH_TIMEOUT(reader->property("viewReady").toBool(), 15000);
        QCOMPARE(reader->property("documentLoads").toInt(), 1);
        QSignalSpy frames(&window, &QQuickWindow::frameSwapped);
        auto profile = reader->property("privateProfile").value<QQuickWebEngineProfile *>();
        QVERIFY(profile && profile->isOffTheRecord());
        QCOMPARE(profile->httpCacheType(), QQuickWebEngineProfile::NoCache);

        // A settings/modal cover freezes the actual WebEngine page. The old
        // Linux adapter ignored this property entirely.
        reader->setProperty("paused", true);
        QTRY_VERIFY(reader->property("documentFrozen").toBool());
        reader->setWidth(550);
        QTest::qWait(160);
        QVERIFY(reader->property("needsMeasurement").toBool());
        reader->setProperty("paused", false);
        QTRY_VERIFY(!reader->property("documentFrozen").toBool());
        QTRY_VERIFY(!reader->property("needsMeasurement").toBool());
        QCOMPARE(reader->property("documentLoads").toInt(), 1);

        for (int cycle = 0; cycle < 10; ++cycle) {
            expose(window, false);
            QVERIFY(window.isVisible());
            QVERIFY(window.visibility() != QWindow::Minimized);
            QTRY_VERIFY(!reader->windowRenderable());
            QTRY_VERIFY(reader->property("documentFrozen").toBool());
            QCOMPARE(reader->property("documentLoads").toInt(), 1);
            if (cycle == 0) {
                QTest::qWait(200); // allow a previously queued frame to retire
                const auto before = frames.count();
                QTest::qWait(200);
                QCOMPARE(frames.count(), before);
            }
            expose(window, true);
            QTRY_VERIFY(reader->windowRenderable());
            QTRY_VERIFY(!reader->property("documentFrozen").toBool());
            QVERIFY(reader->property("viewReady").toBool());
            QCOMPARE(reader->property("documentLoads").toInt(), 1);
        }
        // Multiple document changes during suspension must not start hidden
        // navigations, or replay stale versions when the workspace returns.
        expose(window, false);
        QTRY_VERIFY(reader->property("documentFrozen").toBool());
        reader->setProperty("page", "<html><body>Superseded hidden update</body></html>");
        reader->setProperty("page", "<html><body style='height:900px'>Latest hidden update</body></html>");
        QTest::qWait(120);
        QCOMPARE(reader->property("documentLoads").toInt(), 1);
        expose(window, true);
        QTRY_VERIFY_WITH_TIMEOUT(reader->property("viewReady").toBool(), 10000);
        QCOMPARE(reader->property("documentLoads").toInt(), 2);
        QVERIFY(!reader->property("documentFrozen").toBool());
        delete reader;
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
    }
};
int main(int argc, char **argv) {
    QCoreApplication::setAttribute(Qt::AA_DisableShaderDiskCache);
    QtWebEngineQuick::initialize();
    QGuiApplication app(argc, argv);
    qmlRegisterType<HtmlSurface>("Test.Fluid", 1, 0, "HtmlSurface");
    ReaderLifecycleTest test;
    return QTest::qExec(&test, argc, argv);
}
#include "reader_lifecycle_test.moc"
