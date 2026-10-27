#include "bridge.h"
#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QDesktopServices>
#include <QUrl>
#include <QTimer>
#include <QDir>
#include <QFileInfo>
#include <QApplication>
#include <QStyleHints>
#include <QPalette>
#include <QColor>
#include <QEvent>
#include <QFileDialog>
#include <QColorDialog>
#include <QClipboard>
#include <QMimeData>
#ifdef Q_OS_WIN
#include <windows.h>
#endif

Bridge::Bridge(bool preview,QObject*p):QObject(p),m_preview(preview){
    systemPalette();
    paletteDebounce=new QTimer(this);paletteDebounce->setSingleShot(true);paletteDebounce->setInterval(120);
    connect(&paletteWatcher,&QFileSystemWatcher::directoryChanged,this,[this]{paletteDebounce->start();});
    connect(&paletteWatcher,&QFileSystemWatcher::fileChanged,this,[this]{paletteDebounce->start();});
    connect(paletteDebounce,&QTimer::timeout,this,[this]{const auto file=configRoot+"/noctalia.toml";if(!configRoot.isEmpty()&&QFileInfo::exists(file)&&!paletteWatcher.files().contains(file))paletteWatcher.addPath(file);if(m_settings["theme"]=="noctalia")send("themeReload");});
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
void Bridge::consume(){pending+=process.readAllStandardOutput();while(true){auto n=pending.indexOf('\n');if(n<0)break;if(n>16*1024*1024){process.kill();pending.clear();return;}auto bytes=pending.left(n);pending.remove(0,n+1);QJsonParseError e;auto event=QJsonDocument::fromJson(bytes,&e);if(e.error==QJsonParseError::NoError)receive(event.object());}if(pending.size()>16*1024*1024){process.kill();pending.clear();}}
void Bridge::receive(const QJsonObject&e){
    const auto kind=e["event"].toString();const auto d=e["data"].toObject();const auto id=e["id"].toInteger();
    if(kind.startsWith("clipboard")||kind=="error")clipboardEvent(kind,d,quint64(id));
    if(profileEvent(kind,d,quint64(id))){emit received(kind=="error"?"photoError":kind,d.toVariantMap());return;}
    if(kind=="ready"){m_settings=d["settings"].toObject().toVariantMap();configRoot=d["configPath"].toString();if(QDir(configRoot).exists()){paletteWatcher.addPath(configRoot);auto file=configRoot+"/noctalia.toml";if(QFileInfo::exists(file))paletteWatcher.addPath(file);}systemPalette();if(m_settings["theme"]=="noctalia")send("themeReload");emit settingsChanged();emit ready();if(!m_preview)setView("inbox");}
    else if(kind=="settings"){m_settings=d.toVariantMap();systemPalette();emit settingsChanged();}
    else if(kind=="theme"){if(d.contains("background")){m_palette=d.toVariantMap();emit paletteChanged();}else systemPalette();}
    else if(kind=="authUrl"){openLink(d["url"].toString());}
    else if(kind=="viewReset"){if(quint64(id)<viewRequest)return;m_inbox.replace({});m_messages.replace({});m_readerOpen=false;m_reader.clear();emit readerChanged();}
    else if(kind=="rows"){if(quint64(d["generation"].toInteger())<viewRequest)return;if(d["reset"].toBool())m_inbox.replace(d["items"].toArray());else m_inbox.append(d["items"].toArray());m_more=d["more"].toBool();m_activity="Your inbox, with room to breathe";m_activityKind="idle";emit activityChanged();emit viewChanged();}
    else if(kind=="thread"){if(quint64(id)<readerRequest)return;documentRequests.clear();m_reader=d.toVariantMap();m_messages.replace(d["messages"].toArray());emit readerChanged();}
    else if(kind=="body"){
        const auto key=m_reader.value("key").toMap();
        if(!m_readerOpen || quint64(d["reading"].toInteger())!=readerRequest || d["account"].toString()!=key.value("account").toString())return;
        m_messages.patch(d["id"].toString(),d);
        m_messages.trimBodies(d["id"].toString());
    }
    else if(kind=="readerDocument"){
        const auto key=m_reader.value("key").toMap();const auto message=d["id"].toString();
        if(!m_readerOpen||quint64(d["reading"].toInteger())!=readerRequest||d["account"].toString()!=key.value("account").toString()||documentRequests.value(message)!=quint64(id))return;
        m_messages.patch(message,{{"document",d["html"]},{"externalLoaded",d["externalLoaded"]},{"failedImages",d["failedImages"]},{"documentPending",false}});m_messages.trimBodies(message);
    }
    else if(kind=="rowChanged"){if(d.contains("generation")&&quint64(d["generation"].toInteger())<viewRequest)return;m_inbox.update(d["row"].toObject(),d["insert"].toBool());pruneSelection();}
    else if(kind=="syncRemoved"){if(quint64(d["generation"].toInteger())<viewRequest)return;m_inbox.removeKeys(d["keys"].toArray());pruneSelection();}
    else if(kind=="threadUpdated"){if(!m_readerOpen||quint64(d["reading"].toInteger())!=readerRequest||d["key"].toObject()!=QJsonObject::fromVariantMap(m_reader.value("key").toMap()))return;m_messages.mergeMessages(d["messages"].toArray());}
    else if(kind=="messageState"){if(!m_readerOpen||quint64(d["reading"].toInteger())!=readerRequest)return;m_messages.patch(d["id"].toString(),d);}
    else if(kind=="sync"){m_sync=d.toVariantMap();emit syncChanged();}
    else if(kind=="mutation"){m_inbox.removeKeys(d["removed"].toArray());for(auto v:d["changed"].toArray())m_inbox.update(v.toObject());m_canUndo=d["undo"].toBool();clearSelection();emit activityChanged();}
    else if(kind=="activity"){m_activity=d["text"].toString();m_activityKind=d["kind"].toString();emit activityChanged();}
    else if(kind=="error"){quitWaiting=false;m_error=d["message"].toString();m_activityKind="error";emit errorChanged();emit activityChanged();}
    else if(kind=="composeOpened"){m_composer=d.toVariantMap();emit composerChanged();emit composeRequested(d["mode"].toString());}
    else if(kind=="compose"||kind=="composeClosed"){
        if(d["session"].toString()!=m_composer.value("session").toString())return;
        m_composer.remove("document");
        if(kind=="composeClosed"||d["state"].toString()=="sent")m_composer.clear();
        for(auto i=d.begin();i!=d.end();++i)m_composer[i.key()]=i.value().toVariant();
        emit composerChanged();if(kind=="composeClosed")emit composeClosed();
    }
    else if(kind=="composeFocus"){if(d["session"].toString()==m_composer.value("session").toString())emit composeFocused();}
    else if(kind=="composeCloseQuestion"){if(d["session"].toString()==m_composer.value("session").toString()){emit showRequested();emit composeCloseQuestion(d.toVariantMap());}}
    else if(kind=="quitCancelled"){quitWaiting=false;}
    else if(kind=="quitReady"){
        if(!quitWaiting)return;quitting=true;send("quit");process.closeWriteChannel();
        QTimer::singleShot(5000,this,[this]{if(process.state()!=QProcess::NotRunning)process.kill();});
    }
    else if(kind=="notification"){emit notification(d.toVariantMap());}
    emit received(kind,d.toVariantMap());
}
void Bridge::setView(const QString&folder,const QString&account,const QString&query){m_folder=folder;m_account=account;m_search=query;clearSelection();m_readerOpen=false;openedKey.clear();m_reader.clear();emit readerChanged();emit viewChanged();QVariantMap d{{"folder",folder},{"search",query}};if(!account.isEmpty())d["accounts"]=QVariantList{account};viewRequest=send("view",d);}
void Bridge::selectRow(int row,int modifiers,qreal x,qreal y,qreal w,qreal h,const QVariantMap &projection){
    auto item=m_inbox.at(row);if(item.isEmpty())return;
    auto key=RecordModel::key(QJsonObject::fromVariantMap(item));
    if(!anchorKey.isEmpty())anchor=m_inbox.indexFor(anchorKey);
    // Range selection takes priority after Ctrl-click has entered selection mode.
    if((modifiers&Qt::ShiftModifier)&&anchor>=0){
        m_selecting=true;
        if(!(modifiers&Qt::ControlModifier))m_selection.clear();
        for(int i=qMin(anchor,row);i<=qMax(anchor,row);++i)m_selection.insert(RecordModel::key(QJsonObject::fromVariantMap(m_inbox.at(i))));
        emit selectionChanged();return;
    }
    if((modifiers&Qt::ControlModifier)||m_selecting){
        m_selecting=true;if(m_selection.contains(key))m_selection.remove(key);else m_selection.insert(key);
        anchor=row;anchorKey=key;emit selectionChanged();return;
    }
    anchor=row;anchorKey=key;m_selection.clear();m_selecting=false;emit selectionChanged();
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
void Bridge::pruneSelection(){if(m_selection.removeIf([this](const QString&key){return m_inbox.indexFor(key)<0;}))emit selectionChanged();}
void Bridge::closeReader(){if(m_readerOpen)emit returnRow(m_openedRow);m_readerOpen=false;openedKey.clear();m_messages.replace({});documentRequests.clear();m_reader.clear();emit readerChanged();send("releaseReader");}
void Bridge::openThread(const QString&account,const QString&thread){if(account.isEmpty()||thread.isEmpty())return;clearSelection();const auto key=account+QChar(0)+thread;const auto index=m_inbox.indexFor(key);emit showRequested();if(index>=0){emit openRowRequested(index);return;}m_readerOpen=true;openedKey=key;m_reader={{"key",QVariantMap{{"account",account},{"id",thread}}},{"subject","Opening email"}};m_messages.replace({});documentRequests.clear();emit readerChanged();readerRequest=send("select",{{"account",account},{"id",thread}});}
void Bridge::mutate(const QString&a,const QVariantMap&r){QVariantList keys;if(m_selection.isEmpty()){auto key=r.isEmpty()?m_reader.value("key").toMap():r.value("key").toMap();if(!key.isEmpty())keys.append(key);}else{for(int i=0;i<m_inbox.rowCount();++i){auto row=m_inbox.at(i);if(isSelected(row))keys.append(row["key"]);}}if(keys.isEmpty())return;send("mutate",{{"keys",keys},{"action",a}});}
void Bridge::requestCompose(const QString&mode,const QVariantMap&m){send("composeStart",{{"mode",mode},{"message",m},{"key",m_reader["key"]},{"account",m_account}});}
void Bridge::renderMessage(const QVariantMap&parameters){const auto message=parameters.value("id").toString();documentRequests[message]=send("renderMessage",parameters);m_messages.patch(message,{{"documentPending",true}});}
void Bridge::openLink(const QString&raw){QUrl u(raw);if(u.isValid()&&(u.scheme()=="https"||u.scheme()=="http"||u.scheme()=="mailto"))QDesktopServices::openUrl(u);}
void Bridge::quit(int exitCode){
    if(quitting||quitWaiting)return;quitWaiting=true;quitCode=exitCode;
    if(process.state()==QProcess::NotRunning){QCoreApplication::exit(exitCode);return;}
    emit flushComposer();send("composePrepareQuit");
}
void Bridge::attachFiles(){
    const auto session=m_composer.value("session").toString();if(session.isEmpty())return;
    auto dialog=new QFileDialog;dialog->setAttribute(Qt::WA_DeleteOnClose);dialog->setWindowTitle("Attach files");dialog->setFileMode(QFileDialog::ExistingFiles);dialog->setWindowModality(Qt::ApplicationModal);
    connect(dialog,&QFileDialog::filesSelected,this,[this,session](const QStringList&files){if(m_composer.value("session")==session&&m_composer.value("state")!="closed"&&m_composer.value("state")!="sent")send("composeAttach",{{"session",session},{"paths",files}});});dialog->open();
}
void Bridge::chooseColour(bool background){
    const auto session=m_composer.value("session").toString();auto dialog=new QColorDialog;dialog->setAttribute(Qt::WA_DeleteOnClose);dialog->setWindowModality(Qt::ApplicationModal);
    connect(dialog,&QColorDialog::colorSelected,this,[this,session,background](const QColor&colour){if(m_composer.value("session")==session)emit colourChosen(colour.name(),background);});dialog->open();
}
