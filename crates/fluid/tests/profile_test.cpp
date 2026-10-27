#include "profiles.h"
#include <QtTest>
class ProfileTest:public QObject {
    Q_OBJECT
private slots:
    void pixelBoundsAndCircularSource(){
        ProfileImages images;QByteArray pixel;pixel.append(char(12));pixel.append(char(34));pixel.append(char(56));pixel.append(char(255));
        QVERIFY(images.insert("one",pixel,1,1));QSize size;auto image=images.requestImage("one",&size,{});QCOMPARE(size,QSize(96,96));QCOMPARE(image.pixelColor(48,48),QColor(12,34,56));QCOMPARE(image.pixelColor(0,0).alpha(),0);
        QVERIFY(!images.insert("bad",pixel,97,1));QVERIFY(!images.insert("bad",{},1,1));
        images.clear();QCOMPARE(images.bytes(),qint64(0));QVERIFY(!image.isNull());
    }
    void cacheIsBoundedAndReleasesRemovedPixels(){
        ProfileImages images;QByteArray pixels(96*96*4,char(255));for(int i=0;i<128;++i)QVERIFY(images.insert(QString::number(i),pixels,96,96));
        QVERIFY(!images.insert("overflow",pixels,96,96));QCOMPARE(images.count(),128);QVERIFY(images.bytes()<=128*96*96*4);
        images.remove("0");QVERIFY(!images.contains("0"));QVERIFY(images.insert("replacement",pixels,96,96));images.clear();QCOMPARE(images.count(),0);
    }
};
QTEST_MAIN(ProfileTest)
#include "profile_test.moc"
