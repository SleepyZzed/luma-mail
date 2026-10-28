#pragma once
#include <QColor>
#include <QFontDatabase>
#include <QPointer>
#include <QQuickTextDocument>
#include <QTextCursor>
#include <QTextDocument>
#include <QTimer>
#include <QVariantMap>
#include <QImage>
#include <QHash>
#include <QUrl>
#include <QtQmlIntegration>

class PrivateDocument : public QTextDocument {
public:
    using QTextDocument::QTextDocument;
    QHash<QUrl,QImage> approvedImages;
protected:
    QVariant loadResource(int kind,const QUrl&url) override {return kind==QTextDocument::ImageResource&&approvedImages.contains(url)?QVariant(approvedImages.value(url)):QVariant();}
};
class RichDocument : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QQuickTextDocument* document READ document WRITE setDocument NOTIFY documentChanged)
    Q_PROPERTY(qreal uiScale READ uiScale WRITE setUiScale NOTIFY scaleChanged)
    Q_PROPERTY(bool locked READ locked WRITE setLocked NOTIFY formatChanged)
    Q_PROPERTY(bool bold READ bold NOTIFY formatChanged)
    Q_PROPERTY(bool italic READ italic NOTIFY formatChanged)
    Q_PROPERTY(bool underline READ underline NOTIFY formatChanged)
    Q_PROPERTY(QString fontFamily READ fontFamily NOTIFY formatChanged)
    Q_PROPERTY(qreal fontPointSize READ fontPointSize NOTIFY formatChanged)
    Q_PROPERTY(bool canUndo READ canUndo NOTIFY historyChanged)
    Q_PROPERTY(bool canRedo READ canRedo NOTIFY historyChanged)
    Q_PROPERTY(QStringList fonts READ fonts CONSTANT)
    Q_PROPERTY(qint64 undoBytes READ undoBytes NOTIFY historyChanged)
public:
    explicit RichDocument(QObject *parent=nullptr);
    QQuickTextDocument *document()const{return quick;}
    void setDocument(QQuickTextDocument *value);
    QTextDocument *nativeDocument()const{return doc;}
    qreal uiScale()const{return scale;}
    void setUiScale(qreal value);
    bool locked()const{return readOnly;}
    void setLocked(bool value){readOnly=value;emit formatChanged();}
    bool bold()const;bool italic()const;bool underline()const;
    QString fontFamily()const;
    qreal fontPointSize()const;
    bool canUndo()const{return historyAt>0;}
    bool canRedo()const{return historyAt+1<history.size();}
    qint64 undoBytes()const;
    QStringList fonts()const{return QFontDatabase::families();}
    Q_INVOKABLE void selection(int start,int end,int position);
    Q_INVOKABLE void load(const QVariantMap &document);
    Q_INVOKABLE QVariantMap payload()const;
    Q_INVOKABLE QString html()const;
    Q_INVOKABLE void toggle(const QString &style);
    Q_INVOKABLE void family(const QString &family);
    Q_INVOKABLE void pointSize(qreal size);
    Q_INVOKABLE void foreground(const QColor &colour);
    Q_INVOKABLE void highlight(const QColor &colour);
    Q_INVOKABLE void align(int alignment);
    Q_INVOKABLE void lineSpacing(qreal multiple);
    Q_INVOKABLE void list(bool numbered);
    Q_INVOKABLE void indent(int direction);
    Q_INVOKABLE bool paragraphBreak();
    Q_INVOKABLE bool inList()const;
    Q_INVOKABLE bool link(const QString &url,const QString &label);
    Q_INVOKABLE void removeLink(const QString &url);
    Q_INVOKABLE void clearFormat();
    Q_INVOKABLE bool picture(const QString&url,int width=192);
    Q_INVOKABLE bool picturePreview(const QString&url,const QString&base64);
    Q_INVOKABLE bool table(int rows,int columns);
    Q_INVOKABLE bool tableTab(bool backwards=false);
    Q_INVOKABLE void undo();
    Q_INVOKABLE void redo();
    Q_INVOKABLE bool find(const QString &term,bool backwards=false,bool matchCase=false,bool whole=false);
    Q_INVOKABLE int replace(const QString &term,const QString &replacement,bool all=false,bool matchCase=false,bool whole=false);
signals:
    void documentChanged();void scaleChanged();void formatChanged();void historyChanged();
    void edited();void placeCursor(int start,int end);void error(QString message);
private:
    struct Snapshot {QString html;int cursor;bool rich;};
    QTextCursor cursor()const;
    QTextCharFormat currentFormat()const;
    void apply(const QTextCharFormat &format);
    void changed(int position,int removed,int added);
    void checkpoint();
    void restore(int at);
    void scaleFonts(QTextDocument *document,qreal ratio)const;
    void legacy(const QVariantMap &value);
    QPointer<QQuickTextDocument> quick;
    QPointer<PrivateDocument> doc;
    QTextCharFormat pending;
    int start=0,end=0,position=0,lastInserted=-1;
    qreal scale=1;
    bool mute=false,readOnly=false,rich=false;
    QList<Snapshot> history;
    qsizetype historyAt=-1;
    QTimer historyTimer;
};
