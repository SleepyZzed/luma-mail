#include "runtime_cleanup.h"
#include <QtTest>
#include <QDir>
#include <QFile>
class RuntimeTest:public QObject {
    Q_OBJECT
private slots:
    void liveRuntimeIsProtectedThenRemoved(){
        QString path;{ReaderRuntime runtime;QVERIFY(runtime.isValid());path=runtime.path();QVERIFY(QFileInfo::exists(path));ReaderRuntime::recover();QTest::qWait(250);QVERIFY(QFileInfo::exists(path));}
        QTRY_VERIFY_WITH_TIMEOUT(!QFileInfo::exists(path),4000);
    }
    void staleRuntimeIsRecovered(){
        QTemporaryDir stale(QDir::tempPath()+"/luma-webview-XXXXXX");QVERIFY(stale.isValid());stale.setAutoRemove(false);
        QFile metadata(stale.path()+"/synthetic-runtime-metadata");QVERIFY(metadata.open(QIODevice::WriteOnly));metadata.write("no mail content");metadata.close();
        ReaderRuntime::recover();QTRY_VERIFY_WITH_TIMEOUT(!QFileInfo::exists(stale.path()),4000);
    }
};
QTEST_GUILESS_MAIN(RuntimeTest)
#include "runtime_test.moc"
