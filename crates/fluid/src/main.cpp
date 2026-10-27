#include "bridge.h"
#include "motion_smoke.h"
#include <QApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QQuickItem>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QTemporaryDir>
#include <QTimer>
#include <QIcon>
void startComposerSmoke(Bridge*,QQuickWindow*);

int main(int argc,char**argv){
    QCoreApplication::setAttribute(Qt::AA_DisableShaderDiskCache);
    qputenv("QT_QUICK_CONTROLS_STYLE","Basic");
    qputenv("QML_DISABLE_DISK_CACHE","1");
    QQuickStyle::setStyle("Basic");
    QApplication app(argc,argv);app.setApplicationName("Luma Mail");app.setOrganizationName("Luma Mail");app.setApplicationVersion("0.10.0");
    app.setStyle("Fusion");app.setWindowIcon(QIcon(":/brand/io.github.lumamail.Luma.svg"));
    bool preview=app.arguments().contains("--design-preview");
    QTemporaryDir isolated;if(preview&&qEnvironmentVariableIsEmpty("LUMA_TEST_ROOT"))qputenv("LUMA_TEST_ROOT",isolated.path().toUtf8());
    Bridge bridge(preview);QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("Mail",&bridge);
    engine.rootContext()->setContextProperty("isWindows",
#ifdef Q_OS_WIN
        true
#else
        false
#endif
    );
    engine.loadFromModule("Luma.Fluid","Main");if(engine.rootObjects().isEmpty())return 1;
    auto window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());
    if(preview&&app.arguments().contains("--motion-test"))new MotionSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--composer-test"))startComposerSmoke(&bridge,window);
    QSystemTrayIcon tray(app.windowIcon());QMenu menu;menu.addAction("Open Luma Mail",window,[window]{window->show();window->raise();window->requestActivate();});menu.addSeparator();menu.addAction("Quit",&bridge,[&bridge]{bridge.quit();});tray.setContextMenu(&menu);tray.setToolTip("Luma Mail");
    QObject::connect(&tray,&QSystemTrayIcon::activated,window,[window](QSystemTrayIcon::ActivationReason r){if(r==QSystemTrayIcon::Trigger||r==QSystemTrayIcon::DoubleClick){window->show();window->raise();window->requestActivate();}});tray.show();
    engine.rootContext()->setContextProperty("trayAvailable",QSystemTrayIcon::isSystemTrayAvailable());
    QObject::connect(&bridge,&Bridge::notification,&tray,[&tray](QVariantMap n){tray.showMessage(n["title"].toString(),n["body"].toString());});
    QTimer::singleShot(0,&bridge,&Bridge::start);
    if(preview&&app.arguments().contains("--preview-reader")){
        QObject::connect(&bridge,&Bridge::ready,&app,[&]{if(app.arguments().contains("--preview-account")){if(auto nav=window->findChild<QQuickItem*>("navigation"))nav->setProperty("accountOpen","preview");bridge.setView("inbox","preview");}QTimer::singleShot(350,&app,[&]{auto list=window->findChild<QQuickItem*>("mailList");if(list&&bridge.inbox()->rowCount()>1){auto p=list->mapToScene(QPointF(0,73));bridge.selectRow(1,0,p.x(),p.y(),list->width(),68);}});});
    }
    if(app.arguments().contains("--smoke-test")){QTimer::singleShot(2500,&app,[&]{if(auto path=qEnvironmentVariable("LUMA_SMOKE_SCREENSHOT");!path.isEmpty())window->grabWindow().save(path);bridge.quit();});}
    return app.exec();
}
