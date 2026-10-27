#include "bridge.h"
#include "motion_smoke.h"
#include "profiles.h"
#include "reader_profile.h"
#include "runtime_cleanup.h"
#include "attachment_preview.h"
#ifdef Q_OS_LINUX
#include "notifications_linux.h"
#endif
#ifdef LUMA_WEBENGINE
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#endif
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
void startProfileSmoke(Bridge*,QQuickWindow*);
void startHtmlSmoke(Bridge*,QQuickWindow*);
void startSyncSmoke(Bridge*,QQuickWindow*);
void startAttachmentSmoke(Bridge*,AttachmentPreview*,QQuickWindow*);

int main(int argc,char**argv){
    QCoreApplication::setAttribute(Qt::AA_DisableShaderDiskCache);
#ifdef LUMA_WEBENGINE
    qputenv("QTWEBENGINE_CHROMIUM_FLAGS",qgetenv("QTWEBENGINE_CHROMIUM_FLAGS")+" --disable-gpu-shader-disk-cache --disable-background-networking --disable-component-update --no-first-run");
    QtWebEngineQuick::initialize();
#endif
    qputenv("QT_QUICK_CONTROLS_STYLE","Basic");
    qputenv("QML_DISABLE_DISK_CACHE","1");
    QQuickStyle::setStyle("Basic");
    QApplication app(argc,argv);app.setApplicationName("Luma Mail");app.setOrganizationName("Luma Mail");app.setApplicationVersion("0.10.0");
    app.setDesktopFileName("io.github.lumamail.Luma");
    app.setStyle("Fusion");app.setWindowIcon(QIcon(":/brand/io.github.lumamail.Luma.svg"));
    ReaderRuntime::recover();
    bool preview=app.arguments().contains("--design-preview");
    QTemporaryDir isolated;if(preview&&qEnvironmentVariableIsEmpty("LUMA_TEST_ROOT"))qputenv("LUMA_TEST_ROOT",isolated.path().toUtf8());
    Bridge bridge(preview);AttachmentPreview attachmentPreview(&bridge);bridge.setAttachmentPreview(&attachmentPreview);QQmlApplicationEngine engine;
    QObject::connect(&app,&QCoreApplication::aboutToQuit,&attachmentPreview,&AttachmentPreview::close);
    engine.addImageProvider("attachment",new PreviewImageProvider(attachmentPreview.pixels()));
    engine.rootContext()->setContextProperty("AttachmentViewer",&attachmentPreview);
#ifdef LUMA_WEBENGINE
    engine.rootContext()->setContextProperty("ReaderProfile",createReaderProfile(&engine));
#endif
    auto profiles=new ProfileImages;engine.addImageProvider("profiles",profiles);bridge.setProfileImages(profiles);
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
    auto visibility=[&]{const bool shown=window->isVisible()&&window->visibility()!=QWindow::Minimized;bridge.setWindowVisible(shown);if(!shown&&attachmentPreview.kind()=="Video")attachmentPreview.pause();};
    visibility();QObject::connect(window,&QWindow::visibleChanged,&bridge,visibility);QObject::connect(window,&QWindow::visibilityChanged,&bridge,visibility);
    if(preview&&app.arguments().contains("--motion-test"))new MotionSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--composer-test"))startComposerSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--photo-test"))startProfileSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--html-test"))startHtmlSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--sync-test"))startSyncSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--attachment-test"))startAttachmentSmoke(&bridge,&attachmentPreview,window);
    QSystemTrayIcon tray(app.windowIcon());QMenu menu;menu.addAction("Open Luma Mail",window,[window]{window->show();window->raise();window->requestActivate();});menu.addSeparator();menu.addAction("Quit",&bridge,[&bridge]{bridge.quit();});tray.setContextMenu(&menu);tray.setToolTip("Luma Mail");
    QObject::connect(&tray,&QSystemTrayIcon::activated,window,[window](QSystemTrayIcon::ActivationReason r){if(r==QSystemTrayIcon::Trigger||r==QSystemTrayIcon::DoubleClick){window->show();window->raise();window->requestActivate();}});tray.show();
    engine.rootContext()->setContextProperty("trayAvailable",QSystemTrayIcon::isSystemTrayAvailable());
#ifdef Q_OS_LINUX
    LinuxNotifications notifications;
    QObject::connect(&bridge,&Bridge::notification,&notifications,&LinuxNotifications::notify);
    QObject::connect(&notifications,&LinuxNotifications::activated,&bridge,&Bridge::openThread);
    QObject::connect(&notifications,&LinuxNotifications::status,&bridge,&Bridge::setNotificationStatus);
#else
    QVariantMap lastNotification;
    QObject::connect(&bridge,&Bridge::notification,&tray,[&](QVariantMap n){if(!QSystemTrayIcon::isSystemTrayAvailable()||!QSystemTrayIcon::supportsMessages()){bridge.setNotificationStatus("Windows system notifications are unavailable.");return;}lastNotification=n;tray.showMessage(n["title"].toString(),n["body"].toString());bridge.setNotificationStatus(QString());});
    QObject::connect(&tray,&QSystemTrayIcon::messageClicked,&bridge,[&]{bridge.openThread(lastNotification["account"].toString(),lastNotification["thread"].toString());});
#endif
    QTimer::singleShot(0,&bridge,&Bridge::start);
    if(preview&&app.arguments().contains("--preview-reader")){
        QObject::connect(&bridge,&Bridge::ready,&app,[&]{if(app.arguments().contains("--preview-account")){if(auto nav=window->findChild<QQuickItem*>("navigation"))nav->setProperty("accountOpen","preview");bridge.setView("inbox","preview");}QTimer::singleShot(350,&app,[&]{auto list=window->findChild<QQuickItem*>("mailList");if(list&&bridge.inbox()->rowCount()>1){auto p=list->mapToScene(QPointF(0,73));bridge.selectRow(1,0,p.x(),p.y(),list->width(),68);}});});
    }
    if(app.arguments().contains("--smoke-test")){QTimer::singleShot(2500,&app,[&]{if(auto path=qEnvironmentVariable("LUMA_SMOKE_SCREENSHOT");!path.isEmpty())window->grabWindow().save(path);bridge.quit();});}
    return app.exec();
}
