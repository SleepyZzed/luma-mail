#include "bridge.h"
#include "single_instance.h"
#include <QStandardPaths>
#include <QDesktopServices>
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
void startNavigationSmoke(Bridge*,QQuickWindow*);
void startSearchSmoke(Bridge*,QQuickWindow*);
void startConflictSmoke(Bridge*,QQuickWindow*);
void startResourceSmoke(Bridge*,QQuickWindow*);
void startSignatureSmoke(Bridge*,QQuickWindow*);
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
    app.setQuitOnLastWindowClosed(false);
    ReaderRuntime::recover();
    bool preview=app.arguments().contains("--design-preview");
    QString mailto;for(const auto&argument:app.arguments())if(argument.startsWith("mailto:",Qt::CaseInsensitive)){mailto=argument;break;}
    std::unique_ptr<SingleInstance>instance;
    if(!preview){instance=std::make_unique<SingleInstance>(QStandardPaths::writableLocation(QStandardPaths::AppConfigLocation));const auto result=instance->start(mailto);if(result==SingleInstance::Forwarded)return 0;if(result==SingleInstance::Busy){qWarning()<<"The existing Luma window is busy. Try opening it again shortly.";return 1;}}
    QTemporaryDir isolated;if(preview&&qEnvironmentVariableIsEmpty("LUMA_TEST_ROOT"))qputenv("LUMA_TEST_ROOT",isolated.path().toUtf8());
    Bridge bridge(preview);AttachmentPreview attachmentPreview(&bridge);bridge.setAttachmentPreview(&attachmentPreview);QQmlApplicationEngine engine;
    QObject::connect(&app,&QCoreApplication::aboutToQuit,&attachmentPreview,&AttachmentPreview::close);
    engine.addImageProvider("attachment",new PreviewImageProvider(attachmentPreview.pixels()));
    engine.rootContext()->setContextProperty("AttachmentViewer",&attachmentPreview);
#ifdef LUMA_WEBENGINE
    engine.rootContext()->setContextProperty("ReaderProfiles",new ReaderProfiles(&engine));
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
    // Window visibility can transiently become Windowed while X11 maps a
    // maximized window. Preserve the real native state, not that map event.
    Qt::WindowState restoredState=Qt::WindowNoState;
    QObject::connect(window,&QWindow::windowStateChanged,window,[window,&restoredState](Qt::WindowState state){
        if(window->isVisible()&&!(state&Qt::WindowMinimized))restoredState=state;
    });
    QObject::connect(&bridge,&Bridge::showRequested,window,[window,&restoredState]{
        const auto state=restoredState;
        if(state&Qt::WindowFullScreen)window->showFullScreen();
        else if(state&Qt::WindowMaximized)window->showMaximized();
        else window->showNormal();
        window->raise();window->requestActivate();
    });
    QDesktopServices::setUrlHandler("mailto",&bridge,"handleMailto");
    if(instance)QObject::connect(instance.get(),&SingleInstance::activated,&bridge,[&bridge](const QString&uri){if(uri.isEmpty())emit bridge.showRequested();else bridge.queueMailto(uri);});
    if(!mailto.isEmpty())bridge.queueMailto(mailto);
    auto visibility=[&]{const bool shown=window->isVisible()&&window->visibility()!=QWindow::Minimized;bridge.setWindowVisible(shown);if(!shown&&attachmentPreview.kind()=="Video")attachmentPreview.pause();};
    visibility();QObject::connect(window,&QWindow::visibleChanged,&bridge,visibility);QObject::connect(window,&QWindow::visibilityChanged,&bridge,visibility);
    if(preview&&app.arguments().contains("--motion-test"))new MotionSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--composer-test"))startComposerSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--photo-test"))startProfileSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--html-test"))startHtmlSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--search-test"))startSearchSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--conflict-test"))startConflictSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--resource-test"))startResourceSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--signature-test"))startSignatureSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--navigation-test"))startNavigationSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--sync-test"))startSyncSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--attachment-test"))startAttachmentSmoke(&bridge,&attachmentPreview,window);
    QSystemTrayIcon tray(app.windowIcon());QMenu menu;menu.addAction("Open Luma Mail",&bridge,[&bridge]{emit bridge.showRequested();});menu.addSeparator();menu.addAction("Quit",&bridge,[&bridge]{bridge.quit();});tray.setContextMenu(&menu);tray.setToolTip("Luma Mail");
    QObject::connect(&tray,&QSystemTrayIcon::activated,&bridge,[&bridge](QSystemTrayIcon::ActivationReason r){if(r==QSystemTrayIcon::Trigger||r==QSystemTrayIcon::DoubleClick)emit bridge.showRequested();});tray.show();
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
