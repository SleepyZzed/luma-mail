#include "rich_document.h"
#include "read_document.h"
#include <QtTest>
#include <QTextBlock>
#include <QTextList>
#include <QTextTable>
#include <QBuffer>
#include <QAccessible>
#include <QQmlComponent>
#include <QQmlEngine>
class DocumentTest:public QObject {
    Q_OBJECT
private slots:
    void accessibleDocumentReplacement_data(){QTest::addColumn<bool>("rich");QTest::newRow("reading")<<false;QTest::newRow("composition")<<true;}
    void accessibleDocumentReplacement(){
        QFETCH(bool,rich);QQmlEngine engine;QQmlComponent component(&engine);
        component.setData("import QtQuick\nTextEdit { width:300;height:80;text: 'Original document';Accessible.role:Accessible.EditableText }",QUrl());
        QScopedPointer<QObject> editor(component.create());QVERIFY2(editor,qPrintable(component.errorString()));
        auto wrapper=editor->property("textDocument").value<QQuickTextDocument*>();QVERIFY(wrapper);
        auto previous=QAccessible::queryAccessibleInterface(editor.data());QVERIFY(previous);QVERIFY(previous->textInterface());
        const auto oldId=QAccessible::uniqueId(previous);QCOMPARE(previous->textInterface()->text(0,8),QString("Original"));
        QPointer<QTextDocument> retained;
        {
            ReadDocument reader;RichDocument composer;
            if(rich){composer.load({{"kind","plain"},{"text","Replacement document"}});composer.setDocument(wrapper);retained=composer.nativeDocument();}
            else{reader.setSource("Replacement document");reader.setDocument(wrapper);retained=reader.nativeDocument();}
            QVERIFY(!QAccessible::accessibleInterface(oldId));
            auto current=QAccessible::queryAccessibleInterface(editor.data());QVERIFY(current);QVERIFY(current->textInterface());
            QCOMPARE(current->textInterface()->text(0,current->textInterface()->characterCount()),QString("Replacement document"));
            QVERIFY(!retained->resource(QTextDocument::ImageResource,QUrl("file:///private")).isValid());
            QVERIFY(!retained->resource(QTextDocument::ImageResource,QUrl("https://example.invalid/image.png")).isValid());
        }
        QVERIFY(retained);QCOMPARE(wrapper->textDocument()->toPlainText(),QString("Replacement document"));
        editor.reset();QVERIFY(!retained);
    }
    void paragraphFormattingKeepsIndependentIndentation(){
        RichDocument d;d.load({{"kind","plain"},{"text","First\nSecond"}});auto doc=d.nativeDocument();auto first=doc->begin();auto second=first.next();QTextCursor a(first),b(second);QTextBlockFormat af,bf;af.setLeftMargin(24);bf.setLeftMargin(48);a.setBlockFormat(af);b.setBlockFormat(bf);d.selection(0,doc->characterCount()-1,0);d.align(Qt::AlignRight);d.lineSpacing(1.5);QCOMPARE(first.blockFormat().leftMargin(),qreal(24));QCOMPARE(second.blockFormat().leftMargin(),qreal(48));QCOMPARE(second.blockFormat().lineHeight(),qreal(150));QCOMPARE(second.blockFormat().alignment(),Qt::AlignRight);
    }
    void hostedPicturesStayBoundedAndScaleLogically(){
        RichDocument d;d.load({{"kind","plain"},{"text","Name"}});QImage image(768,576,QImage::Format_ARGB32);image.fill(Qt::cyan);QByteArray png;QBuffer buffer(&png);buffer.open(QIODevice::WriteOnly);QVERIFY(image.save(&buffer,"PNG"));const auto data=QString::fromLatin1(png.toBase64());
        QVERIFY(!d.picture("file:///private/picture.png"));QVERIFY(!d.picturePreview("file:///private/picture.png",data));
        for(int i=0;i<8;++i)QVERIFY(d.picturePreview("https://example.com/"+QString::number(i)+".png",data));QVERIFY(!d.picturePreview("https://example.com/ninth.png",data));
        QVERIFY(d.picture("https://example.com/0.png",192));const auto before=d.html();QVERIFY(before.contains("width=\"192\""));d.setUiScale(1.5);QVERIFY(d.html().contains("width=\"192\""));QVERIFY(!d.html().contains("data:image"));
        auto resource=d.nativeDocument()->resource(QTextDocument::ImageResource,QUrl("https://example.com/0.png"));QVERIFY(resource.value<QImage>().width()<=256);QVERIFY(resource.value<QImage>().height()<=192);
    }
    void tableNavigationIsBoundedAndUndoable(){
        RichDocument d;d.load({{"kind","plain"},{"text","Hello"}});QVERIFY(!d.table(100,100));QVERIFY(d.table(1,2));auto table=QTextCursor(d.nativeDocument()->begin()).currentTable();QVERIFY(d.html().contains("<table"));QVERIFY(d.tableTab());QVERIFY(d.tableTab());QVERIFY(d.tableTab(true));d.undo();QVERIFY(d.undoBytes()<=1024*1024);Q_UNUSED(table);
    }
    void tableTabLeavesAtCapacityAndShiftTabDoesNotGrow(){
        RichDocument d;d.load({{"kind","plain"},{"text",""}});QVERIFY(d.table(1,2));
        QTextCursor c(d.nativeDocument());int caret=0;connect(&d,&RichDocument::placeCursor,this,[&](int,int end){caret=end;});
        for(int i=0;i<19;++i)QVERIFY(d.tableTab());
        c.setPosition(caret);auto table=c.currentTable();QVERIFY(table);QCOMPARE(table->rows(),10);QCOMPARE(table->columns(),2);
        QCOMPARE(table->cellAt(c).row(),9);QCOMPARE(table->cellAt(c).column(),1);
        QVERIFY(d.tableTab(true));c.setPosition(caret);QCOMPARE(table->cellAt(c).column(),0);QCOMPARE(table->rows(),10);
        QVERIFY(d.tableTab());QVERIFY(d.tableTab());c.setPosition(caret);QVERIFY(!c.currentTable());QCOMPARE(table->rows(),10);
        QVERIFY(!d.tableTab());d.undo();QVERIFY(d.undoBytes()<=1024*1024);
    }
    void receivedLinksAreLiteralSafeAndSelectionCompatible(){
        ReadDocument reader;const QString source=QString::fromUtf8("<b>literal</b> 🙂 https://example.com and mail@example.com");reader.setSource(source);
        const auto start=source.indexOf("https://");reader.setSpans({QVariantMap{{"start",start},{"end",start+19},{"url","https://example.com"}},QVariantMap{{"start",0},{"end",3},{"url","file:///private"}}});reader.setLinkColour(QColor("#82b5ff"));
        auto doc=reader.nativeDocument();QCOMPARE(doc->toPlainText(),source);QTextCursor cursor(doc);cursor.setPosition(start+2);QVERIFY(cursor.charFormat().isAnchor());QCOMPARE(cursor.charFormat().anchorHref(),QString("https://example.com"));QCOMPARE(cursor.charFormat().foreground().color(),QColor("#82b5ff"));
        cursor.setPosition(2);QVERIFY(!cursor.charFormat().isAnchor());reader.setForeground(Qt::white);QCOMPARE(doc->toPlainText(),source);QVERIFY(!doc->resource(QTextDocument::ImageResource,QUrl("file:///private")).isValid());
    }
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
