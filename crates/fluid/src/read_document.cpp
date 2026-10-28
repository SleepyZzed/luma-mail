#include "read_document.h"
#include "document_binding.h"
#include <QUrl>
#include <cmath>
ReadDocument::ReadDocument(QObject*parent):QObject(parent),doc(new PrivateDocument(this)){fontFamily=QFontDatabase::systemFont(QFontDatabase::GeneralFont).family();doc->setUndoRedoEnabled(false);doc->setDocumentMargin(0);doc->documentLayout();}
void ReadDocument::setDocument(QQuickTextDocument*value){if(quick==value)return;quick=value;if(quick)bindPrivateDocument(quick,doc);emit changed();}
void ReadDocument::setSource(const QString&value){if(text==value)return;text=value;format();emit changed();}
void ReadDocument::setSpans(const QVariantList&value){if(links==value)return;links=value.mid(0,1000);format();emit changed();}
void ReadDocument::setForeground(QColor value){if(colour==value||!value.isValid())return;colour=value;format();emit changed();}
void ReadDocument::setLinkColour(QColor value){if(blue==value||!value.isValid())return;blue=value;format();emit changed();}
void ReadDocument::setFamily(const QString&value){if(fontFamily==value)return;fontFamily=value;format();emit changed();}
void ReadDocument::setPixelSize(qreal value){if(!std::isfinite(value)||value<6||value>96||pixels==value)return;pixels=value;format();emit changed();}
void ReadDocument::format(){
    if(!doc)return;if(doc->toPlainText()!=text)doc->setPlainText(text);QFont font(fontFamily);font.setPixelSize(qRound(pixels));doc->setDefaultFont(font);
    QTextCursor cursor(doc);cursor.select(QTextCursor::Document);QTextCharFormat normal;normal.setForeground(colour);cursor.setCharFormat(normal);
    for(const auto&span:links){const auto s=span.toMap();const int start=s["start"].toInt(),end=s["end"].toInt();const QUrl url(s["url"].toString());
        if(start<0||end<=start||end>text.size()||!url.isValid()||!(url.scheme()=="https"||url.scheme()=="http"||url.scheme()=="mailto"))continue;
        cursor.setPosition(start);cursor.setPosition(end,QTextCursor::KeepAnchor);QTextCharFormat anchor;anchor.setAnchor(true);anchor.setAnchorHref(url.toString());anchor.setForeground(blue);anchor.setFontUnderline(true);cursor.mergeCharFormat(anchor);
    }
}
