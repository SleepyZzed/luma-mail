#pragma once
#include "rich_document.h"
class ReadDocument:public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QQuickTextDocument* document READ document WRITE setDocument NOTIFY changed)
    Q_PROPERTY(QString source READ source WRITE setSource NOTIFY changed)
    Q_PROPERTY(QVariantList spans READ spans WRITE setSpans NOTIFY changed)
    Q_PROPERTY(QColor foreground READ foreground WRITE setForeground NOTIFY changed)
    Q_PROPERTY(QColor linkColour READ linkColour WRITE setLinkColour NOTIFY changed)
    Q_PROPERTY(QString family READ family WRITE setFamily NOTIFY changed)
    Q_PROPERTY(qreal pixelSize READ pixelSize WRITE setPixelSize NOTIFY changed)
public:
    explicit ReadDocument(QObject*parent=nullptr);
    QQuickTextDocument*document()const{return quick;}void setDocument(QQuickTextDocument*value);
    QString source()const{return text;}void setSource(const QString&value);
    QVariantList spans()const{return links;}void setSpans(const QVariantList&value);
    QColor foreground()const{return colour;}void setForeground(QColor value);
    QColor linkColour()const{return blue;}void setLinkColour(QColor value);
    QString family()const{return fontFamily;}void setFamily(const QString&value);
    qreal pixelSize()const{return pixels;}void setPixelSize(qreal value);
    QTextDocument*nativeDocument()const{return doc;}
signals:void changed();
private:
    void format();QPointer<QQuickTextDocument>quick;QPointer<PrivateDocument>doc;
    QString text,fontFamily;QVariantList links;QColor colour=Qt::black,blue=QColor("#165fcb");qreal pixels=13;
};
