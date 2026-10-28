#include "single_instance.h"
#include <QCoreApplication>
#include <QTest>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QProcess>
#include <QLocalSocket>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTextStream>
class InstanceTest:public QObject{
    Q_OBJECT
private slots:
    void forwardsToOneOwnerAndRejectsOtherCommands(){
        QTemporaryDir root;SingleInstance owner(root.path());QCOMPARE(owner.start(),SingleInstance::Primary);QSignalSpy received(&owner,&SingleInstance::activated);
        QProcess child;const QString uri="mailto:alex+tag@example.com?subject=Test%20mail";child.start(QCoreApplication::applicationFilePath(),{"--forward",root.path(),uri});
        QTRY_COMPARE_WITH_TIMEOUT(child.state(),QProcess::NotRunning,5000);QCOMPARE(child.exitCode(),0);QCOMPARE(received.size(),1);QCOMPARE(received.first().first().toString(),uri);
        QLocalSocket bad;bad.connectToServer(owner.endpoint());QVERIFY(bad.waitForConnected(500));bad.write(QJsonDocument(QJsonObject{{"version",1},{"uri","file:///private"}}).toJson(QJsonDocument::Compact)+'\n');bad.flush();
        QTRY_COMPARE_WITH_TIMEOUT(bad.state(),QLocalSocket::UnconnectedState,3000);QCOMPARE(received.size(),1);
    }
    void recoversOnlyTheStaleOwnedEndpointAfterCrash(){
        QTemporaryDir root;QProcess child;child.start(QCoreApplication::applicationFilePath(),{"--own",root.path()});
        QTRY_VERIFY_WITH_TIMEOUT(child.bytesAvailable()>0,3000);QVERIFY(child.readAllStandardOutput().startsWith("ready"));child.kill();QVERIFY(child.waitForFinished(3000));
        SingleInstance recovered(root.path());QCOMPARE(recovered.start(),SingleInstance::Primary);
    }
};
int main(int argc,char**argv){
    QCoreApplication app(argc,argv);app.setApplicationName("Luma Instance Test");app.setOrganizationName("Luma Test");const auto args=app.arguments();
    if(args.size()>=3&&(args[1]=="--forward"||args[1]=="--own")){
        SingleInstance instance(args[2]);const auto result=instance.start(args.size()>3?args[3]:QString());
        if(args[1]=="--forward")return result==SingleInstance::Forwarded?0:2;
        if(result!=SingleInstance::Primary)return 3;QTextStream(stdout)<<"ready\n"<<Qt::flush;return app.exec();
    }
    InstanceTest test;return QTest::qExec(&test,argc,argv);
}
#include "instance_test.moc"
