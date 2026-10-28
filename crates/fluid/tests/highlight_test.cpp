#include "search_highlights.h"
#include <QtTest>
class HighlightTest:public QObject{
    Q_OBJECT
private slots:
    void untrustedHeadersStayLiteral(){SearchHighlights h;auto text=h.render("<img src='https://invalid'> launch & details","launch",QColor("#aa55cc"));QVERIFY(!text.contains("<img"));QVERIFY(text.contains("&lt;img"));QVERIFY(text.contains("<b>launch</b>"));QVERIFY(text.contains("&amp; details"));}
    void operatorsAndNegativesAreNotGuessedAsPositiveWords(){SearchHighlights h;const auto value=h.render("Alice spam launch project","from:Alice -spam has:attachment subject:launch project",Qt::blue);QVERIFY(!value.contains("<b>Alice"));QVERIFY(!value.contains("<b>spam"));QVERIFY(value.contains("<b>launch</b>"));QVERIFY(value.contains("<b>project</b>"));}
    void quotedUnicodeAndRegexCharactersRemainLiteral(){SearchHighlights h;auto text=h.render("MÜNCHEN launch [a+b]",QString::fromUtf8("\"münchen launch\" [a+b]"),Qt::blue);QVERIFY(text.contains("<b>MÜNCHEN launch</b>"));QVERIFY(text.contains("<b>[a+b]</b>"));}
    void retargetingDropsThePreviousQuery(){SearchHighlights h;QVERIFY(h.render("alpha beta","alpha",Qt::blue).contains("<b>alpha</b>"));auto text=h.render("alpha beta","beta",Qt::blue);QVERIFY(!text.contains("<b>alpha"));QVERIFY(text.contains("<b>beta</b>"));QCOMPARE(h.render("alpha beta","",Qt::blue),QString("alpha beta"));}
    void outputAndMatchWorkAreBounded(){SearchHighlights h;auto text=h.render(QString("word ").repeated(5000),"word",Qt::blue);QCOMPARE(text.count("<b>"),32);QVERIFY(text.size()<12000);QVERIFY(text.endsWith(QChar(0x2026)));QCOMPARE(h.render("alpha",QString(8193,'a'),Qt::blue),QString("alpha"));}
    void longerPhrasesWinAndMalformedQuotesRemainPlain(){SearchHighlights h;QVERIFY(h.render("alpha beta","alpha \"alpha beta\"",Qt::blue).contains("<b>alpha beta</b>"));QCOMPARE(h.render("alpha","\"alpha",Qt::blue),QString("alpha"));}
};
QTEST_GUILESS_MAIN(HighlightTest)
#include "highlight_test.moc"
