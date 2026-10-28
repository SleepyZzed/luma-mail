#include "notifications_windows.h"
#include <QtTest>
#include <QJsonDocument>
#include <QJsonObject>
namespace {int released=0;QList<QJsonObject>payloads;WindowsNotifications::Callback received=nullptr;
void*show(const unsigned char*data,size_t size,WindowsNotifications::Callback callback,quintptr token){payloads.append(QJsonDocument::fromJson(QByteArray(reinterpret_cast<const char*>(data),qsizetype(size))).object());received=callback;return reinterpret_cast<void*>(token);}
void release(void*){++released;}
bool identity(){return true;}
}
class NotificationTest:public QObject {
    Q_OBJECT
private slots:
    void independentActionsAndBoundedOwnership(){
        released=0;payloads.clear();WindowsNotifications adapter(qApp,{show,release,identity});QSignalSpy opened(&adapter,&WindowsNotifications::activated),copied(&adapter,&WindowsNotifications::copyCode),status(&adapter,&WindowsNotifications::status);
        adapter.notify({{"account","a"},{"thread","first"},{"title","First"}});
        adapter.notify({{"account","b"},{"thread","second"},{"title","Code"},{"code","007341"}});
        QVERIFY(!payloads[0]["copyCode"].toBool());QVERIFY(payloads[1]["copyCode"].toBool());QVERIFY(!QJsonDocument(payloads[1]).toJson().contains("007341"));
        received(1,reinterpret_cast<const unsigned char*>("open"),4);QTRY_COMPARE(opened.size(),1);QCOMPARE(opened[0][0].toString(),QString("a"));QCOMPARE(opened[0][1].toString(),QString("first"));
        received(2,reinterpret_cast<const unsigned char*>("copy"),4);QTRY_COMPARE(copied.size(),1);QCOMPARE(copied[0][0].toString(),QString("007341"));QCOMPARE(opened.size(),1);QCOMPARE(released,2);
        adapter.activate(2,"copy");QCOMPARE(copied.size(),1);
        adapter.notify({{"account","b"},{"thread","third"}});const auto nonce=payloads.last()["activation"].toString();adapter.activateArguments("open:00000000-0000-0000-0000-000000000000");QCOMPARE(opened.size(),1);adapter.activateArguments("open:"+nonce);QCOMPARE(opened.size(),2);QCOMPARE(opened.last()[1].toString(),QString("third"));QCOMPARE(released,3);
        for(int i=0;i<65;++i)adapter.notify({{"account","a"},{"thread",QString::number(i)}});
        QCOMPARE(released,4);adapter.activate(4,"open");QCOMPARE(opened.size(),2);adapter.clear();QCOMPARE(released,68);
        QVERIFY(status.last()[0].toString().isEmpty());
    }
};
QTEST_GUILESS_MAIN(NotificationTest)
#include "notifications_windows_test.moc"
