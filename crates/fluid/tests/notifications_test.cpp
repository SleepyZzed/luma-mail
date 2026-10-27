#include "notifications_linux.h"
#include <QtTest>
#include <QDBusConnection>
#include <QDBusMessage>
class Daemon:public QObject {
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface","org.freedesktop.Notifications")
public:
    QString summary,body;QStringList actions;QVariantMap hints;int calls=0;
public slots:
    uint Notify(const QString&,uint,const QString&,const QString&title,const QString&text,const QStringList&buttons,const QVariantMap&metadata,int){summary=title;body=text;actions=buttons;hints=metadata;++calls;return 42;}
};
class NotificationTest:public QObject {
    Q_OBJECT
private slots:
    void routesToTheDaemonAndOpensTheCorrectThread(){
        Daemon daemon;auto bus=QDBusConnection::sessionBus();QVERIFY(bus.registerService("org.freedesktop.Notifications"));QVERIFY(bus.registerObject("/org/freedesktop/Notifications",&daemon,QDBusConnection::ExportAllSlots));
        LinuxNotifications adapter;QSignalSpy activation(&adapter,&LinuxNotifications::activated);QSignalSpy status(&adapter,&LinuxNotifications::status);
        adapter.notify({{"account","a"},{"thread","thread1"},{"title","New email"},{"body","<b>Sender-controlled text</b>"}});
        QTRY_COMPARE(daemon.calls,1);QTRY_COMPARE(status.size(),1);QVERIFY(status.last()[0].toString().isEmpty());QCOMPARE(daemon.body,QString("&lt;b&gt;Sender-controlled text&lt;/b&gt;"));QCOMPARE(daemon.actions,QStringList({"default","Open email"}));QCOMPARE(daemon.hints["desktop-entry"].toString(),QString("io.github.lumamail.Luma"));
        auto clicked=QDBusMessage::createSignal("/org/freedesktop/Notifications","org.freedesktop.Notifications","ActionInvoked");clicked.setArguments({uint(42),"default"});QVERIFY(bus.send(clicked));QTRY_COMPARE(activation.size(),1);QCOMPARE(activation.last()[0].toString(),QString("a"));QCOMPARE(activation.last()[1].toString(),QString("thread1"));
        bus.unregisterObject("/org/freedesktop/Notifications");bus.unregisterService("org.freedesktop.Notifications");
    }
};
QTEST_GUILESS_MAIN(NotificationTest)
#include "notifications_test.moc"
