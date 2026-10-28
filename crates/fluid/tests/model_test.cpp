#include "record_model.h"
#include <QtTest>
#include <QAbstractItemModelTester>
class ModelTest:public QObject {
    Q_OBJECT
    static QJsonObject row(int id,int date){return {{"key",QJsonObject{{"account","a"},{"id",QString::number(id)}}},{"date",date},{"subject","Synthetic"}};}
private slots:
    void accountUpdatesKeepIdentityAndOrderWithoutResettingDelegates(){
        RecordModel model;QAbstractItemModelTester tester(&model,QAbstractItemModelTester::FailureReportingMode::Fatal);QSignalSpy resets(&model,&QAbstractItemModel::modelReset);
        const QJsonObject a{{"id","a"},{"name","First"}},b{{"id","b"},{"name","Second"}};
        model.reconcileOrdered({a,b});QPersistentModelIndex second(model.index(1));
        model.reconcileOrdered({QJsonObject{{"id","b"},{"name","Renamed"}},a});
        QVERIFY(second.isValid());QCOMPARE(second.row(),0);QCOMPARE(model.at(0)["name"].toString(),QString("Renamed"));QCOMPARE(resets.size(),0);
        QSignalSpy changes(&model,&QAbstractItemModel::dataChanged);model.reconcileOrdered({QJsonObject{{"id","b"},{"name","Renamed"}},a});QCOMPARE(changes.size(),0);
        model.reconcileOrdered({a});QVERIFY(!second.isValid());QCOMPARE(model.indexFor("a"),0);
    }
    void incomingRowsMoveAndStayWithinTheLoadedLimit(){
        RecordModel model;QAbstractItemModelTester tester(&model,QAbstractItemModelTester::FailureReportingMode::Fatal);QJsonArray rows;for(int i=0;i<1000;++i)rows.append(row(i,1000-i));model.replace(rows);
        auto key=RecordModel::key(row(500,0));model.update(row(500,2000),true);QCOMPARE(model.rowCount(),1000);QCOMPARE(model.indexFor(key),0);QCOMPARE(model.at(0)["key"].toMap()["id"].toString(),QString("500"));
        model.update(row(1001,3000),true);QCOMPARE(model.rowCount(),1000);QCOMPARE(model.at(0)["key"].toMap()["id"].toString(),QString("1001"));QCOMPARE(model.indexFor(RecordModel::key(row(999,0))),-1);QCOMPARE(model.indexFor(key),1);
        model.update(row(2000,-1),true);QCOMPARE(model.rowCount(),1000);QCOMPARE(model.indexFor(RecordModel::key(row(2000,0))),-1);
    }
    void replyHeadersPreserveOpenedBodies(){
        RecordModel model;model.replace({QJsonObject{{"id","m1"},{"text","Opened body"},{"document","Private HTML"},{"unread",false}}});
        model.mergeMessages({QJsonObject{{"id","m1"},{"starred",true}},QJsonObject{{"id","m2"},{"unread",true}}});
        QCOMPARE(model.at(0)["text"].toString(),QString("Opened body"));QCOMPARE(model.at(0)["document"].toString(),QString("Private HTML"));QVERIFY(model.at(1)["arrived"].toBool());QVERIFY(model.at(1)["unread"].toBool());
        model.patch("m2",{{"unread",false}});QVERIFY(!model.at(1)["unread"].toBool());
    }
    void movingHeaderWindowsKeepOpenBodiesAndReleaseDepartedMessages(){
        RecordModel model;QAbstractItemModelTester tester(&model,QAbstractItemModelTester::FailureReportingMode::Fatal);
        QJsonArray headers;for(int i=0;i<32;++i)headers.append(QJsonObject{{"id",QString::number(i)}});model.windowMessages(headers,false);model.patch("31",{{"text","Kept body"},{"document","Kept HTML"}});model.patch("0",{{"text","Evicted body"}});
        QPersistentModelIndex retained(model.index(31));headers.removeAt(0);headers.append(QJsonObject{{"id","32"},{"unread",true}});model.windowMessages(headers,true);
        QCOMPARE(model.rowCount(),32);QVERIFY(retained.isValid());QCOMPARE(retained.row(),30);QCOMPARE(model.indexFor("0"),-1);QCOMPARE(model.at(30)["text"].toString(),QString("Kept body"));QVERIFY(model.at(31)["arrived"].toBool());QVERIFY(!model.at(31).contains("text"));
        model.windowMessages({QJsonObject{{"id","older"}}},false);QCOMPARE(model.rowCount(),1);QVERIFY(!model.at(0).contains("text"));QVERIFY(!model.at(0)["arrived"].toBool());
    }
    void readerPayloadsAreEvictedWithTheirLinkMetadata(){
        RecordModel model;QJsonArray rows;for(int i=0;i<9;++i)rows.append(QJsonObject{{"id",QString::number(i)},{"text","Opened body"},{"links",QJsonArray{QJsonObject{{"url","https://example.com"}}}}});model.replace(rows);model.trimBodies("8");
        QVERIFY(!model.at(0).contains("text"));QVERIFY(!model.at(0).contains("links"));QCOMPARE(model.at(8)["text"].toString(),QString("Opened body"));
    }
};
QTEST_GUILESS_MAIN(ModelTest)
#include "model_test.moc"
