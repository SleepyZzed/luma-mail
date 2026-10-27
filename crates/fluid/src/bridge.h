#pragma once
#include <QAbstractListModel>
#include <QProcess>
#include <QJsonObject>
#include <QJsonArray>
#include <QVariantMap>
#include <QSet>
#include <QFileSystemWatcher>
#include <QTimer>

class RecordModel : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
public:
    explicit RecordModel(QObject *parent=nullptr):QAbstractListModel(parent){}
    int rowCount(const QModelIndex &p={}) const override {return p.isValid()?0:rows.size();}
    QVariant data(const QModelIndex &i,int role) const override;
    QHash<int,QByteArray> roleNames() const override {return {{Qt::UserRole+1,"item"}};}
    Q_INVOKABLE QVariantMap at(int i) const {return i>=0&&i<rows.size()?rows[i].toVariantMap():QVariantMap{};}
    void replace(const QJsonArray &items);
    void append(const QJsonArray &items);
    void update(const QJsonObject &item,bool insert=false);
    void removeKeys(const QJsonArray &keys);
    void patch(const QString &id,const QJsonObject &values);
    static QString key(const QJsonObject &row);
signals: void countChanged();
private: QList<QJsonObject> rows;
};

class Bridge : public QObject {
    Q_OBJECT
    Q_PROPERTY(RecordModel* inbox READ inbox CONSTANT)
    Q_PROPERTY(RecordModel* messages READ messages CONSTANT)
    Q_PROPERTY(QVariantMap settings READ settings NOTIFY settingsChanged)
    Q_PROPERTY(QVariantMap reader READ reader NOTIFY readerChanged)
    Q_PROPERTY(QVariantMap composer READ composer NOTIFY composerChanged)
    Q_PROPERTY(QVariantMap palette READ palette NOTIFY paletteChanged)
    Q_PROPERTY(QString folder READ folder NOTIFY viewChanged)
    Q_PROPERTY(QString account READ account NOTIFY viewChanged)
    Q_PROPERTY(QString search READ search NOTIFY viewChanged)
    Q_PROPERTY(QString activity READ activity NOTIFY activityChanged)
    Q_PROPERTY(QString activityKind READ activityKind NOTIFY activityChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(bool readerOpen READ readerOpen NOTIFY readerChanged)
    Q_PROPERTY(bool more READ more NOTIFY viewChanged)
    Q_PROPERTY(bool canUndo READ canUndo NOTIFY activityChanged)
    Q_PROPERTY(int selectionCount READ selectionCount NOTIFY selectionChanged)
    Q_PROPERTY(bool selecting READ selecting NOTIFY selectionChanged)
    Q_PROPERTY(bool preview READ preview CONSTANT)
public:
    explicit Bridge(bool preview,QObject *parent=nullptr);
    void start();
    RecordModel* inbox(){return &m_inbox;} RecordModel* messages(){return &m_messages;}
    QVariantMap settings()const{return m_settings;} QVariantMap reader()const{return m_reader;} QVariantMap composer()const{return m_composer;}
    QVariantMap palette()const{return m_palette;}
    QString folder()const{return m_folder;} QString search()const{return m_search;}
    QString account()const{return m_account;}
    QString activity()const{return m_activity;} QString activityKind()const{return m_activityKind;} QString error()const{return m_error;}
    bool readerOpen()const{return m_readerOpen;} bool more()const{return m_more;} bool canUndo()const{return m_canUndo;}
    int selectionCount()const{return m_selection.size();} bool selecting()const{return m_selecting;} bool preview()const{return m_preview;}
    Q_INVOKABLE quint64 send(const QString &action,const QVariantMap &data={});
    Q_INVOKABLE void setView(const QString &folder,const QString &account=QString(),const QString &search=QString());
    Q_INVOKABLE void selectRow(int row,int modifiers,qreal x,qreal y,qreal width,qreal height,const QVariantMap &projection={});
    Q_INVOKABLE bool isSelected(const QVariantMap &row)const;
    Q_INVOKABLE void selectionMode(bool enabled);
    Q_INVOKABLE void selectAll();
    Q_INVOKABLE void clearSelection();
    Q_INVOKABLE void closeReader();
    Q_INVOKABLE void mutate(const QString &action,const QVariantMap &row={});
    Q_INVOKABLE void requestCompose(const QString &mode="new",const QVariantMap &message={});
    Q_INVOKABLE void openLink(const QString &url);
    Q_INVOKABLE void attachFiles();
    Q_INVOKABLE void chooseColour(bool background=false);
    Q_INVOKABLE bool interceptImagePaste();
    Q_INVOKABLE void clearError(){m_error.clear();emit errorChanged();}
    Q_INVOKABLE void quit(int exitCode=0);
signals:
    void settingsChanged(); void readerChanged(); void composerChanged(); void viewChanged();
    void paletteChanged();
    void activityChanged(); void errorChanged(); void selectionChanged();
    void projectRow(QVariantMap row,qreal x,qreal y,qreal width,qreal height);
    void returnRow(QVariantMap row);
    void composeRequested(QString mode); void notification(QVariantMap data);
    void composeFocused();void composeClosed();void composeCloseQuestion(QVariantMap data);
    void flushComposer();void colourChosen(QString colour,bool background);void showRequested();
    void received(QString event,QVariantMap data); void ready();
private:
    void consume(); void receive(const QJsonObject &event);
    void systemPalette();
    bool eventFilter(QObject *object,QEvent *event) override;
    RecordModel m_inbox{this},m_messages{this}; QProcess process; QByteArray pending;
    QVariantMap m_settings,m_reader,m_composer,m_palette; QString m_folder="inbox",m_account,m_search;
    QVariantMap m_openedRow;
    QFileSystemWatcher paletteWatcher; QTimer *paletteDebounce=nullptr;
    QString m_activity="Connecting to your inbox",m_activityKind="loading",m_error;
    QSet<QString> m_selection; int anchor=-1; bool m_selecting=false,m_readerOpen=false,m_more=false,m_canUndo=false,m_preview;
    quint64 serial=0,viewRequest=0,readerRequest=0; QString openedKey;
    bool quitting=false,quitWaiting=false; int quitCode=0;
};
