#include "bridge.h"
#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QDesktopServices>
#include <QUrl>
#include <QTimer>
#include <QDir>
#include <QApplication>
#include <QStyleHints>
#include <QPalette>
#include <QColor>
#include <QEvent>
#ifdef Q_OS_WIN
#include <windows.h>
#endif

QVariant RecordModel::data(const QModelIndex&i,int role)const{return i.isValid()&&i.row()<rows.size()&&role==Qt::UserRole+1?QVariant(rows[i.row()].toVariantMap()):QVariant{};}
QString RecordModel::key(const QJsonObject&r){auto k=r["key"].toObject();return k.isEmpty()?r["id"].toString():k["account"].toString()+QChar(0)+k["id"].toString();}
void RecordModel::replace(const QJsonArray&a){beginResetModel();rows.clear();for(auto v:a){if(rows.size()==1000)break;rows.append(v.toObject());}endResetModel();emit countChanged();}
void RecordModel::append(const QJsonArray&a){QList<QJsonObject>fresh;QSet<QString>seen;for(auto&r:rows)seen.insert(key(r));for(auto v:a){auto r=v.toObject();auto k=key(r);if(!seen.contains(k)&&rows.size()+fresh.size()<1000){fresh.append(r);seen.insert(k);}}if(fresh.isEmpty())return;beginInsertRows({},rows.size(),rows.size()+fresh.size()-1);rows.append(fresh);endInsertRows();emit countChanged();}
void RecordModel::update(const QJsonObject&r,bool insert){auto k=key(r);for(int i=0;i<rows.size();++i)if(key(rows[i])==k){rows[i]=r;emit dataChanged(index(i),index(i),{Qt::UserRole+1});return;}if(insert&&rows.size()<1000){int at=0;while(at<rows.size()&&rows[at]["date"].toInteger()>r["date"].toInteger())++at;beginInsertRows({},at,at);rows.insert(at,r);endInsertRows();emit countChanged();}}
void RecordModel::removeKeys(const QJsonArray&a){QSet<QString>keys;for(auto k:a){QJsonObject r;r["key"]=k;keys.insert(key(r));}for(int i=rows.size()-1;i>=0;--i)if(keys.contains(key(rows[i]))){beginRemoveRows({},i,i);rows.removeAt(i);endRemoveRows();}emit countChanged();}
void RecordModel::patch(const QString&id,const QJsonObject&v){for(int i=0;i<rows.size();++i)if(rows[i]["id"].toString()==id){for(auto it=v.begin();it!=v.end();++it)rows[i][it.key()]=it.value();emit dataChanged(index(i),index(i),{Qt::UserRole+1});return;}}

Bridge::Bridge(bool preview,QObject*p):QObject(p),m_preview(preview){
    systemPalette();
    paletteDebounce=new QTimer(this);paletteDebounce->setSingleShot(true);paletteDebounce->setInterval(120);
    connect(&paletteWatcher,&QFileSystemWatcher::directoryChanged,this,[this]{paletteDebounce->start();});
    connect(&paletteWatcher,&QFileSystemWatcher::fileChanged,this,[this]{paletteDebounce->start();});
    connect(paletteDebounce,&QTimer::timeout,this,[this]{if(m_settings["theme"]=="noctalia")send("themeReload");});
    qApp->installEventFilter(this);
    connect(qApp->styleHints(),&QStyleHints::colorSchemeChanged,this,[this]{systemPalette();});
    connect(&process,&QProcess::readyReadStandardOutput,this,&Bridge::consume);
    connect(&process,&QProcess::errorOccurred,this,[this]{m_error="The mail engine could not start. Check the complete application package.";emit errorChanged();});
    connect(&process,qOverload<int,QProcess::ExitStatus>(&QProcess::finished),this,[this](int code,QProcess::ExitStatus status){if(quitting){QCoreApplication::exit(quitCode);return;}if(code!=0||status!=QProcess::NormalExit){m_error="The mail engine stopped. Your mail stays in Gmail; restart Luma to reconnect.";emit errorChanged();}});
#ifdef Q_OS_WIN
    process.setCreateProcessArgumentsModifier([](QProcess::CreateProcessArguments*a){a->flags|=CREATE_NO_WINDOW;});
#endif
}
bool Bridge::eventFilter(QObject *object,QEvent *event){
    if(object==qApp && event->type()==QEvent::ApplicationPaletteChange)systemPalette();
    return QObject::eventFilter(object,event);
}
void Bridge::systemPalette(){
    if(m_settings.value("theme").toString()=="noctalia")return;
    auto mode=m_settings.value("theme","system").toString();
    const bool dark=mode=="dark"||(mode=="system"&&qApp->styleHints()->colorScheme()==Qt::ColorScheme::Dark);
    auto accent=qApp->palette().color(QPalette::Highlight);if(accent.lightnessF()<.25)accent=QColor("#b1a0df");
    m_palette={{"mode",dark?"dark":"light"},{"background",dark?"#19181f":"#f6f5fa"},{"surface",dark?"#23212b":"#ffffff"},{"text",dark?"#f2eff7":"#292535"},{"muted",dark?"#a59dae":"#777183"},{"accent",accent.name()},{"accentText",accent.lightnessF()>.6?"#20192e":"#ffffff"},{"outline",dark?"#393440":"#e2deea"},{"hover",dark?"#302b3a":"#f0edf6"}};
    emit paletteChanged();
}
void Bridge::start(){
    auto binary=qEnvironmentVariable("LUMA_ENGINE");if(binary.isEmpty())binary=QCoreApplication::applicationDirPath()+"/luma-engine"
#ifdef Q_OS_WIN
        ".exe"
#endif
        ;
    process.setProgram(binary);process.setArguments(m_preview?QStringList{"--design-preview"}:QStringList{});process.start();
}
quint64 Bridge::send(const QString&a,const QVariantMap&data){if(process.state()!=QProcess::Running)return 0;auto id=++serial;QJsonObject c{{"id",qint64(id)},{"action",a},{"data",QJsonObject::fromVariantMap(data)}};auto bytes=QJsonDocument(c).toJson(QJsonDocument::Compact);if(bytes.size()>4*1024*1024){m_error="The editor input exceeds its memory safety limit.";emit errorChanged();return 0;}process.write(bytes+'\n');return id;}
void Bridge::consume(){pending+=process.readAllStandardOutput();if(pending.size()>16*1024*1024){process.kill();return;}while(true){auto n=pending.indexOf('\n');if(n<0)break;auto bytes=pending.left(n);pending.remove(0,n+1);QJsonParseError e;auto event=QJsonDocument::fromJson(bytes,&e);if(e.error==QJsonParseError::NoError)receive(event.object());}}
void Bridge::receive(const QJsonObject&e){
    const auto kind=e["event"].toString();const auto d=e["data"].toObject();const auto id=e["id"].toInteger();
    if(kind=="ready"){m_settings=d["settings"].toObject().toVariantMap();auto path=d["configPath"].toString();if(QDir(path).exists())paletteWatcher.addPath(path);systemPalette();if(m_settings["theme"]=="noctalia")send("themeReload");emit settingsChanged();emit ready();if(!m_preview)setView("inbox");}
    else if(kind=="settings"){m_settings=d.toVariantMap();systemPalette();emit settingsChanged();}
    else if(kind=="theme"){if(d.contains("background")){m_palette=d.toVariantMap();emit paletteChanged();}else systemPalette();}
    else if(kind=="authUrl"){openLink(d["url"].toString());}
    else if(kind=="viewReset"){if(quint64(id)<viewRequest)return;m_inbox.replace({});m_messages.replace({});m_readerOpen=false;m_reader.clear();emit readerChanged();}
    else if(kind=="rows"){if(quint64(d["generation"].toInteger())<viewRequest)return;if(d["reset"].toBool())m_inbox.replace(d["items"].toArray());else m_inbox.append(d["items"].toArray());m_more=d["more"].toBool();m_activity="Your inbox, with room to breathe";m_activityKind="idle";emit activityChanged();emit viewChanged();}
    else if(kind=="thread"){if(quint64(id)<readerRequest)return;m_reader=d.toVariantMap();m_messages.replace(d["messages"].toArray());emit readerChanged();}
    else if(kind=="body"){
        const auto key=m_reader.value("key").toMap();
        if(!m_readerOpen || quint64(d["reading"].toInteger())!=readerRequest || d["account"].toString()!=key.value("account").toString())return;
        m_messages.patch(d["id"].toString(),d);
    }
    else if(kind=="rowChanged"){m_inbox.update(d["row"].toObject(),d["insert"].toBool());}
    else if(kind=="mutation"){m_inbox.removeKeys(d["removed"].toArray());for(auto v:d["changed"].toArray())m_inbox.update(v.toObject());m_canUndo=d["undo"].toBool();clearSelection();emit activityChanged();}
    else if(kind=="activity"){m_activity=d["text"].toString();m_activityKind=d["kind"].toString();emit activityChanged();}
    else if(kind=="error"){m_error=d["message"].toString();m_activityKind="error";emit errorChanged();emit activityChanged();}
    else if(kind=="compose"){m_composer=d.toVariantMap();emit composerChanged();}
    else if(kind=="notification"){emit notification(d.toVariantMap());}
    emit received(kind,d.toVariantMap());
}
void Bridge::setView(const QString&folder,const QString&account,const QString&query){m_folder=folder;m_account=account;m_search=query;clearSelection();m_readerOpen=false;openedKey.clear();m_reader.clear();emit readerChanged();emit viewChanged();QVariantMap d{{"folder",folder},{"search",query}};if(!account.isEmpty())d["accounts"]=QVariantList{account};viewRequest=send("view",d);}
void Bridge::selectRow(int row,int modifiers,qreal x,qreal y,qreal w,qreal h,const QVariantMap &projection){
    auto item=m_inbox.at(row);if(item.isEmpty())return;
    auto key=RecordModel::key(QJsonObject::fromVariantMap(item));
    // Range selection takes priority after Ctrl-click has entered selection mode.
    if((modifiers&Qt::ShiftModifier)&&anchor>=0){
        m_selecting=true;
        if(!(modifiers&Qt::ControlModifier))m_selection.clear();
        for(int i=qMin(anchor,row);i<=qMax(anchor,row);++i)m_selection.insert(RecordModel::key(QJsonObject::fromVariantMap(m_inbox.at(i))));
        emit selectionChanged();return;
    }
    if((modifiers&Qt::ControlModifier)||m_selecting){
        m_selecting=true;if(m_selection.contains(key))m_selection.remove(key);else m_selection.insert(key);
        anchor=row;emit selectionChanged();return;
    }
    anchor=row;m_selection.clear();m_selecting=false;emit selectionChanged();
    if(openedKey==key&&m_readerOpen)return;
    openedKey=key;m_readerOpen=true;
    item["_projection"]=projection;m_openedRow=item;
    m_reader={{"subject",item["subject"]},{"key",item["key"]}};
    m_messages.replace({});
    emit readerChanged();emit projectRow(item,x,y,w,h);
    readerRequest=send("select",item["key"].toMap());
}
bool Bridge::isSelected(const QVariantMap&r)const{return m_selection.contains(RecordModel::key(QJsonObject::fromVariantMap(r)));}
void Bridge::selectionMode(bool on){m_selecting=on;if(!on)m_selection.clear();emit selectionChanged();}
void Bridge::selectAll(){m_selecting=true;for(int i=0;i<m_inbox.rowCount();++i)m_selection.insert(RecordModel::key(QJsonObject::fromVariantMap(m_inbox.at(i))));emit selectionChanged();}
void Bridge::clearSelection(){m_selection.clear();m_selecting=false;emit selectionChanged();}
void Bridge::closeReader(){if(m_readerOpen)emit returnRow(m_openedRow);m_readerOpen=false;openedKey.clear();emit readerChanged();send("releaseReader");}
void Bridge::mutate(const QString&a,const QVariantMap&r){QVariantList keys;if(m_selection.isEmpty()){auto key=r.isEmpty()?m_reader.value("key").toMap():r.value("key").toMap();if(!key.isEmpty())keys.append(key);}else{for(int i=0;i<m_inbox.rowCount();++i){auto row=m_inbox.at(i);if(isSelected(row))keys.append(row["key"]);}}if(keys.isEmpty())return;send("mutate",{{"keys",keys},{"action",a}});}
void Bridge::requestCompose(const QString&mode,const QVariantMap&m){emit composeRequested(mode);send("composeStart",{{"mode",mode},{"message",m},{"key",m_reader["key"]}});}
void Bridge::openLink(const QString&raw){QUrl u(raw);if(u.isValid()&&(u.scheme()=="https"||u.scheme()=="http"||u.scheme()=="mailto"))QDesktopServices::openUrl(u);}
void Bridge::quit(int exitCode){
    if(quitting)return;quitting=true;quitCode=exitCode;
    if(process.state()==QProcess::NotRunning){QCoreApplication::exit(exitCode);return;}
    send("quit");process.closeWriteChannel();
    QTimer::singleShot(5000,this,[this]{if(process.state()!=QProcess::NotRunning)process.kill();});
}
