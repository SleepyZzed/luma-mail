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
#else
#include "notifications_windows.h"
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
#include <QClipboard>
void startComposerSmoke(Bridge*,QQuickWindow*);
void startProfileSmoke(Bridge*,QQuickWindow*);
void startHtmlSmoke(Bridge*,QQuickWindow*);
void startSyncSmoke(Bridge*,QQuickWindow*);
void startNavigationSmoke(Bridge*,QQuickWindow*);
void startSearchSmoke(Bridge*,QQuickWindow*);
void startConflictSmoke(Bridge*,QQuickWindow*);
void startResourceSmoke(Bridge*,QQuickWindow*);
void startSignatureSmoke(Bridge*,QQuickWindow*);
void startThreadSmoke(Bridge*,QQuickWindow*);
void startPopoutSmoke(Bridge*,QQuickWindow*);
void startAuthSmoke(Bridge*,QQuickWindow*);
void startRecoverySmoke(Bridge*,QQuickWindow*);
void startPopupSmoke(Bridge*,QQuickWindow*);
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
    QApplication app(argc,argv);app.setApplicationName("Luma Mail");app.setOrganizationName("Luma Mail");app.setApplicationVersion("0.11.0");
    app.setDesktopFileName("io.github.lumamail.Luma");
    app.setStyle("Fusion");app.setWindowIcon(QIcon(":/brand/io.github.lumamail.Luma.svg"));
    app.setQuitOnLastWindowClosed(false);
    ReaderRuntime::recover();
    bool preview=app.arguments().contains("--design-preview");
    const bool acceptance=app.arguments().contains("--acceptance-test")&&!qEnvironmentVariableIsEmpty("LUMA_TEST_ROOT");
    const bool traceMotion=preview&&app.arguments().contains("--motion-test");
    if(traceMotion)qInfo()<<"MOTION startup: application initialized";
    QString mailto;for(const auto&argument:app.arguments())if(argument.startsWith("mailto:",Qt::CaseInsensitive)){mailto=argument;break;}
    std::unique_ptr<SingleInstance>instance;
    if(!preview){auto scope=QStandardPaths::writableLocation(QStandardPaths::AppConfigLocation);if(acceptance)scope+="/acceptance/"+qEnvironmentVariable("LUMA_TEST_ROOT");instance=std::make_unique<SingleInstance>(scope);const auto result=instance->start(mailto);if(result==SingleInstance::Forwarded)return 0;if(result==SingleInstance::Busy){qWarning()<<"The existing Luma window is busy. Try opening it again shortly.";return 1;}}
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
    engine.rootContext()->setContextProperty("resourceFixture",preview&&app.arguments().contains("--resource-test"));
    engine.rootContext()->setContextProperty("isWindows",
#ifdef Q_OS_WIN
        true
#else
        false
#endif
    );
    if(traceMotion)qInfo()<<"MOTION startup: loading native scene";
    engine.loadFromModule("Luma.Fluid","Main");if(engine.rootObjects().isEmpty())return 1;
    if(traceMotion)qInfo()<<"MOTION startup: native scene loaded";
    auto window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());
    // Preserve the native state, rather than transient X11 map visibility.
    bridge.trackWindow(window);
    QObject::connect(&bridge,&Bridge::showRequested,window,[&bridge,window]{bridge.activateWindow(window);});
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
    if(preview&&app.arguments().contains("--popup-test"))startPopupSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--auth-test"))startAuthSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--recovery-test"))startRecoverySmoke(&bridge,window);
    if(preview&&app.arguments().contains("--popout-test"))startPopoutSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--thread-test"))startThreadSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--sync-test"))startSyncSmoke(&bridge,window);
    if(preview&&app.arguments().contains("--attachment-test"))startAttachmentSmoke(&bridge,&attachmentPreview,window);
    QSystemTrayIcon tray(app.windowIcon());QMenu menu;menu.addAction("Open Luma Mail",&bridge,[&bridge]{emit bridge.showRequested();});menu.addSeparator();menu.addAction("Quit",&bridge,[&bridge]{bridge.quit();});tray.setContextMenu(&menu);tray.setToolTip("Luma Mail");
    QObject::connect(&tray,&QSystemTrayIcon::activated,&bridge,[&bridge](QSystemTrayIcon::ActivationReason r){if(r==QSystemTrayIcon::Trigger||r==QSystemTrayIcon::DoubleClick)emit bridge.showRequested();});if(!preview&&!acceptance)tray.show();
    engine.rootContext()->setContextProperty("trayAvailable",!preview&&!acceptance&&QSystemTrayIcon::isSystemTrayAvailable());
    QObject::connect(&app,&QCoreApplication::aboutToQuit,&tray,&QSystemTrayIcon::hide);
#ifdef Q_OS_LINUX
    LinuxNotifications notifications;
    QObject::connect(&bridge,&Bridge::notification,&notifications,&LinuxNotifications::notify);
    QObject::connect(&notifications,&LinuxNotifications::activated,&bridge,[&bridge](const QString&account,const QString&thread){if(thread.isEmpty())bridge.showMainWindow();else bridge.openThread(account,thread);});
    QObject::connect(&notifications,&LinuxNotifications::copyCode,&app,[](const QString&code){QApplication::clipboard()->setText(code);});
    QObject::connect(&notifications,&LinuxNotifications::status,&bridge,&Bridge::setNotificationStatus);
#else
    WindowsNotifications notifications(&app);
    QObject::connect(&bridge,&Bridge::notification,&notifications,&WindowsNotifications::notify);
    QObject::connect(&notifications,&WindowsNotifications::activated,&bridge,[&bridge](const QString&account,const QString&thread){if(thread.isEmpty())bridge.showMainWindow();else bridge.openThread(account,thread);});
    QObject::connect(&notifications,&WindowsNotifications::copyCode,&app,[](const QString&code){QApplication::clipboard()->setText(code);});
    QObject::connect(&notifications,&WindowsNotifications::status,&bridge,&Bridge::setNotificationStatus);
#endif
    QTimer::singleShot(0,&bridge,[&bridge,traceMotion]{if(traceMotion)qInfo()<<"MOTION startup: starting synthetic engine";bridge.start();});
    if(preview&&app.arguments().contains("--preview-reader")){
        QObject::connect(&bridge,&Bridge::ready,&app,[&]{if(app.arguments().contains("--preview-account")){if(auto nav=window->findChild<QQuickItem*>("navigation"))nav->setProperty("accountOpen","preview");bridge.setView("inbox","preview");}QTimer::singleShot(350,&app,[&]{auto list=window->findChild<QQuickItem*>("mailList");if(list&&bridge.inbox()->rowCount()>1){auto p=list->mapToScene(QPointF(0,73));bridge.selectRow(1,0,p.x(),p.y(),list->width(),68);}});});
    }
    if(app.arguments().contains("--smoke-test")){QTimer::singleShot(2500,&app,[&]{if(auto path=qEnvironmentVariable("LUMA_SMOKE_SCREENSHOT");!path.isEmpty())window->grabWindow().save(path);bridge.quit();});}
    return app.exec();
}
