#include "bridge.h"
#include "attachment_preview.h"
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
#include <QWindow>
#include <QQuickWindow>
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
void Bridge::trackWindow(QObject*object){
    auto window=qobject_cast<QWindow*>(object);if(!window||window->property("lumaRestoreState").isValid())return;
    window->setProperty("lumaRestoreState",int(Qt::WindowNoState));
    if(auto quick=qobject_cast<QQuickWindow*>(window)){
        // A one-shot background grace period avoids rebuilding graphics for
        // quick hide/restore cycles. QML state and mail sessions stay intact.
        auto idle=new QTimer(quick);idle->setSingleShot(true);idle->setInterval(5000);
        auto visibility=[quick,idle]{if(quick->isVisible()&&quick->visibility()!=QWindow::Minimized){idle->stop();quick->setPersistentGraphics(true);quick->setPersistentSceneGraph(true);}else idle->start();};
        connect(quick,&QWindow::visibleChanged,quick,visibility);connect(quick,&QWindow::visibilityChanged,quick,visibility);
        connect(idle,&QTimer::timeout,quick,[quick]{if(!quick->isVisible()||quick->visibility()==QWindow::Minimized){quick->setPersistentGraphics(false);quick->setPersistentSceneGraph(false);quick->releaseResources();}});
    }

    connect(window,&QWindow::windowStateChanged,window,[window](Qt::WindowState state){if(window->isVisible()&&!(state&Qt::WindowMinimized))window->setProperty("lumaRestoreState",int(state));});
}
void Bridge::activateWindow(QObject*object){
    auto window=qobject_cast<QWindow*>(object);if(!window)return;
    const auto state=window->property("lumaRestoreState").toInt();
    if(state&Qt::WindowFullScreen)window->showFullScreen();else if(state&Qt::WindowMaximized)window->showMaximized();else window->showNormal();window->raise();window->requestActivate();
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
    const QVariantMap next={{"mode",dark?"dark":"light"},{"background",dark?"#19181f":"#f6f5fa"},{"surface",dark?"#23212b":"#ffffff"},{"text",dark?"#f2eff7":"#292535"},{"muted",dark?"#a59dae":"#777183"},{"accent",accent.name()},{"accentText",accent.lightnessF()>.6?"#20192e":"#ffffff"},{"outline",dark?"#393440":"#e2deea"},{"hover",dark?"#302b3a":"#f0edf6"}};
    if(next==m_palette)return;m_palette=next;emit paletteChanged();
}
void Bridge::start(){
    auto binary=qEnvironmentVariable("LUMA_ENGINE");if(binary.isEmpty())binary=QCoreApplication::applicationDirPath()+"/luma-engine"
#ifdef Q_OS_WIN
        ".exe"
#endif
        ;
    process.setProgram(binary);process.setArguments(m_preview?QStringList{"--design-preview"}:QStringList{});process.start();
}
quint64 Bridge::send(const QString&a,const QVariantMap&data){if(process.state()!=QProcess::Running)return 0;auto id=++serial;if(a=="searchBuild")searchRequest=id;if(a=="labels")labelRequests[data.value("account").toString()]=id;QJsonObject c{{"id",qint64(id)},{"action",a},{"data",QJsonObject::fromVariantMap(data)}};auto bytes=QJsonDocument(c).toJson(QJsonDocument::Compact);if(bytes.size()>4*1024*1024){m_error="The editor input exceeds its memory safety limit.";emit errorChanged();return 0;}process.write(bytes+'\n');return id;}
void Bridge::consume(){pending+=process.readAllStandardOutput();while(true){auto n=pending.indexOf('\n');if(n<0)break;if(n>16*1024*1024){process.kill();pending.clear();return;}auto bytes=pending.left(n);pending.remove(0,n+1);QJsonParseError e;auto event=QJsonDocument::fromJson(bytes,&e);if(e.error==QJsonParseError::NoError)receive(event.object());}if(pending.size()>16*1024*1024){process.kill();pending.clear();}}
void Bridge::receive(const QJsonObject&e){
    const auto kind=e["event"].toString();const auto d=e["data"].toObject();const auto id=e["id"].toInteger();
    if(attachmentPreview&&attachmentPreview->consume(kind,d,quint64(id))){emit received(kind=="error"?"attachmentError":kind,QVariantMap{{"token",d["token"].toString()}});return;}
    if(kind.startsWith("clipboard")||kind=="error")clipboardEvent(kind,d,quint64(id));
    if(profileEvent(kind,d,quint64(id))){emit received(kind=="error"?"photoError":kind,d.toVariantMap());return;}
    if(kind=="ready"){m_settings=d["settings"].toObject().toVariantMap();m_accounts.reconcileOrdered(d["settings"].toObject()["accounts"].toArray());configRoot=d["configPath"].toString();if(QDir(configRoot).exists()){paletteWatcher.addPath(configRoot);auto file=configRoot+"/noctalia.toml";if(QFileInfo::exists(file))paletteWatcher.addPath(file);}systemPalette();if(m_settings["theme"]=="noctalia")send("themeReload");emit settingsChanged();engineReady=true;emit ready();if(!m_preview)setView("inbox");flushMailto();}
    else if(kind=="settings"){
        auto enabled=[](const QVariantMap&settings){QSet<QString>ids;for(const auto&value:settings.value("accounts").toList()){const auto account=value.toMap();if(account.value("enabled").toBool())ids.insert(account.value("id").toString());}return ids;};
        const auto previous=enabled(m_settings);m_settings=d.toVariantMap();m_accounts.reconcileOrdered(d["accounts"].toArray());const auto current=enabled(m_settings);
        QSet<QString>present;for(const auto&value:m_settings.value("accounts").toList())present.insert(value.toMap().value("id").toString());
        for(auto i=m_labels.begin();i!=m_labels.end();){if(!present.contains(i.key()))i=m_labels.erase(i);else ++i;}
        systemPalette();emit settingsChanged();emit labelsChanged();
        if(previous!=current){
            if(searchSubset){scopedAccounts.removeIf([&current](const QVariant&v){return !current.contains(v.toString());});m_account=scopedAccounts.size()==1?scopedAccounts.first().toString():QString();if(scopedAccounts.isEmpty())setView("inbox");else refreshView();}
            else setView(m_folder.startsWith("label:")&&!current.contains(m_account)?"inbox":m_folder,current.contains(m_account)?m_account:QString(),m_search);
        }flushMailto();
    }
    else if(kind=="theme"){if(d.contains("background")){m_palette=d.toVariantMap();emit paletteChanged();}else systemPalette();}
    else if(kind=="authUrl"){openLink(d["url"].toString());}
    else if(kind=="searchBuilt"){if(quint64(id)!=searchRequest||quint64(id)<viewRequest)return;applySearch(d);}
    else if(kind=="viewReset"){if(quint64(id)<viewRequest)return;m_inbox.replace({});m_messages.replace({});m_readerOpen=false;m_reader.clear();emit readerChanged();}
    else if(kind=="rows"){if(quint64(d["generation"].toInteger())<viewRequest)return;if(d["reset"].toBool())m_inbox.replace(d["items"].toArray());else m_inbox.append(d["items"].toArray());m_more=d["more"].toBool();m_activity="Your inbox, with room to breathe";m_activityKind="idle";emit activityChanged();emit viewChanged();}
    else if(kind=="thread"){if(quint64(id)!=readerRequest||!m_readerOpen)return;headerRequest=0;documentRequests.clear();m_reader=d.toVariantMap();m_reader.remove("messages");m_messages.windowMessages(d["messages"].toArray(),false);emit readerChanged();}
    else if(kind=="threadHeaders"){
        if(!m_readerOpen||quint64(id)!=headerRequest||quint64(d["reading"].toInteger())!=readerRequest||d["key"].toObject()!=QJsonObject::fromVariantMap(m_reader.value("key").toMap()))return;
        headerRequest=0;documentRequests.clear();m_reader["pagePending"]=false;m_reader["window"]=d["window"].toObject().toVariantMap();m_messages.windowMessages(d["messages"].toArray(),false);emit readerChanged();
    }
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
    else if(kind=="threadUpdated"){
        if(!m_readerOpen||headerRequest||quint64(d["reading"].toInteger())!=readerRequest||d["key"].toObject()!=QJsonObject::fromVariantMap(m_reader.value("key").toMap()))return;
        if(d.contains("window")){
            if(d["window"].toObject()["revision"].toInteger()<m_reader.value("window").toMap().value("revision").toLongLong())return;
            m_reader["window"]=d["window"].toObject().toVariantMap();m_messages.windowMessages(d["messages"].toArray(),true);emit readerChanged();
        }else m_messages.mergeMessages(d["messages"].toArray());
    }
    else if(kind=="messageState"){if(!m_readerOpen||quint64(d["reading"].toInteger())!=readerRequest)return;m_messages.patch(d["id"].toString(),d);}
    else if(kind=="sync"){m_sync=d.toVariantMap();emit syncChanged();}
    else if(kind=="labels"){const auto account=d["account"].toString();if(labelRequests.value(account)>quint64(id))return;labelRequests.remove(account);m_labels[account]=d["items"].toArray().toVariantList();emit labelsChanged();if(d["operation"].toString()=="delete"&&m_account==account&&m_folder=="label:"+d["label"].toString())setView("inbox",account);}
    else if(kind=="mutation"){if(d.contains("generation")&&quint64(d["generation"].toInteger())<viewRequest)return;const auto key=m_reader.value("key").toMap();for(const auto flag:d["messageFlags"].toArray()){auto row=flag.toObject();if(m_readerOpen&&row["account"].toString()==key.value("account").toString()&&row["thread"].toString()==key.value("id").toString()){auto message=row["message"].toObject();m_messages.patch(message["id"].toString(),message);}}m_inbox.removeKeys(d["removed"].toArray());for(auto v:d["changed"].toArray())m_inbox.update(v.toObject());m_canUndo=d["undo"].toBool();clearSelection();emit activityChanged();}
    else if(kind=="activity"){m_activity=d["text"].toString();m_activityKind=d["kind"].toString();emit activityChanged();}
    else if(kind=="error"){if(headerRequest==quint64(id)){headerRequest=0;m_reader["pagePending"]=false;emit readerChanged();}if(labelRequests.removeIf([id](auto entry){return entry.value()==quint64(id);}))emit labelsChanged();quitWaiting=false;m_error=d["message"].toString();m_activityKind="error";emit errorChanged();emit activityChanged();}
    else if(kind=="composeOpened"){m_composer=d.toVariantMap();emit composerChanged();emit composeRequested(d["mode"].toString());}
    else if(kind=="compose"||kind=="composeClosed"){
        if(d["session"].toString()!=m_composer.value("session").toString())return;
        m_composer.remove("document");
        if(kind=="composeClosed")m_composer.clear();
        else{if(d["state"].toString()=="sent")m_composer.clear();for(auto i=d.begin();i!=d.end();++i)m_composer[i.key()]=i.value().toVariant();}
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
void Bridge::pageReader(const QString&direction){if(!m_readerOpen||headerRequest||!m_reader.contains("window"))return;headerRequest=send("threadPage",{{"key",m_reader["key"]},{"reading",qint64(readerRequest)},{"direction",direction}});if(headerRequest){m_reader["pagePending"]=true;emit readerChanged();}}
void Bridge::closeReader(){headerRequest=0;if(m_readerOpen)emit returnRow(m_openedRow);m_readerOpen=false;openedKey.clear();m_messages.replace({});documentRequests.clear();m_reader.clear();emit readerChanged();send("releaseReader");}
void Bridge::openThread(const QString&account,const QString&thread){if(account.isEmpty()||thread.isEmpty())return;clearSelection();const auto key=account+QChar(0)+thread;const auto index=m_inbox.indexFor(key);emit showRequested();if(index>=0){emit openRowRequested(index);return;}m_readerOpen=true;openedKey=key;m_reader={{"key",QVariantMap{{"account",account},{"id",thread}}},{"subject","Opening email"}};m_messages.replace({});documentRequests.clear();emit readerChanged();readerRequest=send("select",{{"account",account},{"id",thread}});}
void Bridge::mutate(const QString&a,const QVariantMap&r,const QString&label){QVariantList keys;if(m_selection.isEmpty()){auto key=r.isEmpty()?m_reader.value("key").toMap():r.value("key").toMap();if(!key.isEmpty())keys.append(key);}else{for(int i=0;i<m_inbox.rowCount();++i){auto row=m_inbox.at(i);if(isSelected(row))keys.append(row["key"]);}}if(keys.isEmpty())return;send("mutate",{{"keys",keys},{"action",a},{"label",label}});}
void Bridge::requestCompose(const QString&mode,const QVariantMap&m){send("composeStart",{{"mode",mode},{"message",m},{"key",m_reader["key"]},{"account",m_account}});}
void Bridge::renderMessage(const QVariantMap&parameters){const auto message=parameters.value("id").toString();documentRequests[message]=send("renderMessage",parameters);m_messages.patch(message,{{"documentPending",true}});}
void Bridge::openLink(const QString&raw){QUrl u(raw);if(u.scheme()=="mailto"){queueMailto(raw);return;}if(u.isValid()&&(u.scheme()=="https"||u.scheme()=="http"||u.scheme()=="mailto"))QDesktopServices::openUrl(u);}
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

void Bridge::ensureLabels(const QString&account,bool force){if(account.isEmpty()||(!force&&(m_labels.contains(account)||labelRequests.contains(account))))return;send("labels",{{"account",account},{"operation","list"}});}
