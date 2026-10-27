#include "rich_document.h"
#include <QtTest>
#include <QTextBlock>
#include <QTextList>
class DocumentTest:public QObject {
    Q_OBJECT
private slots:
    void selectionAndLogicalScaling(){
        RichDocument d;d.load({{"kind","plain"},{"text","Alpha beta"}});
        d.selection(0,5,5);d.toggle("bold");d.foreground(QColor("#123abc"));d.pointSize(14);
        QTextCursor c(d.nativeDocument());c.setPosition(2);QVERIFY(c.charFormat().fontWeight()>=QFont::Bold);
        c.setPosition(8);QVERIFY(c.charFormat().fontWeight()<QFont::Bold);
        auto before=d.html();QVERIFY(before.contains("#123abc"));QVERIFY(before.contains("14pt"));
        d.setUiScale(1.5);auto after=d.html();QVERIFY(after.contains("14pt"));QVERIFY(!after.contains("21pt"));
        QCOMPARE(d.nativeDocument()->toPlainText(),QString("Alpha beta"));
    }
    void paragraphListsAndEmptyExit(){
        RichDocument d;d.load({{"kind","plain"},{"text","First\nSecond"}});d.selection(2,2,2);d.list(false);
        QVERIFY(d.nativeDocument()->begin().textList());QCOMPARE(d.nativeDocument()->toPlainText(),QString("First\nSecond"));
        d.selection(5,5,5);QVERIFY(d.paragraphBreak());QVERIFY(d.inList());QVERIFY(d.paragraphBreak());QVERIFY(!d.inList());
        QVERIFY(d.html().contains("<ul"));
    }
    void linksUndoAndResourceIsolation(){
        RichDocument d;d.load({{"kind","plain"},{"text","Hello"}});d.selection(0,5,5);
        QVERIFY(!d.link("file:///private/file","bad"));QVERIFY(!d.link("javascript:alert(1)","bad"));
        QVERIFY(d.link("https://example.com/a","Example"));QVERIFY(d.html().contains("#1a73e8"));
        d.undo();QCOMPARE(d.nativeDocument()->toPlainText(),QString("Hello"));d.redo();QCOMPARE(d.nativeDocument()->toPlainText(),QString("Example"));
        QVERIFY(d.nativeDocument()->resource(QTextDocument::ImageResource,QUrl("file:///private/file")).isNull());
        QVERIFY(d.nativeDocument()->resource(QTextDocument::ImageResource,QUrl("https://example.com/image.png")).isNull());
        d.locked();d.setLocked(true);d.list(true);QCOMPARE(d.nativeDocument()->toPlainText(),QString("Example"));
    }
    void legacyUnicodeAndBoundedHistory(){
        RichDocument d;d.load({{"kind","legacy"},{"text",QString::fromUtf8("🌟 Alpha")},{"spans",QVariantList{QVariantMap{{"start",2},{"end",7},{"tag","strong"}}}}});
        QCOMPARE(d.nativeDocument()->toPlainText(),QString::fromUtf8("🌟 Alpha"));
        QTextCursor c(d.nativeDocument());c.setPosition(4);QVERIFY(c.charFormat().fontWeight()>=QFont::Bold);c.setPosition(1);QVERIFY(c.charFormat().fontWeight()<QFont::Bold);
        for(int i=0;i<90;++i){d.selection(3,8,8);d.foreground(QColor::fromHsl(i*3,180,120));}
        QVERIFY(d.undoBytes()<=1024*1024);
    }
};
QTEST_MAIN(DocumentTest)
#include "document_test.moc"
