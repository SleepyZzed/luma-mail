// Native synthetic smoke through the real QML form, bridge and Rust child.
#include "bridge.h"
#include "rich_document.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QJsonDocument>
#include <QSaveFile>
#include <QTimer>
#include <QBuffer>
#include <QImage>
#include <QTextTable>
#include <QDir>
#include <QDateTime>
#include <functional>
class ComposerSmoke:public QObject {
    Bridge *mail;QQuickWindow *window;QQuickItem *card=nullptr;QPointer<RichDocument> document=nullptr;
    int stage=0;bool failed=false,finished=false;QString session;QJsonArray checks;
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void invoke(const char*method){QMetaObject::invokeMethod(card,method);}
    void key(int code,Qt::KeyboardModifiers modifiers=Qt::NoModifier){QKeyEvent press(QEvent::KeyPress,code,modifiers);QCoreApplication::sendEvent(window,&press);QKeyEvent release(QEvent::KeyRelease,code,modifiers);QCoreApplication::sendEvent(window,&release);}
    void afterClosed(std::function<void()> next,int attempts=50){
        if(!card->property("shown").toBool()&&!card->property("closingPending").toBool()&&(!document||document->nativeDocument()->isEmpty())){next();return;}
        if(!attempts){check("closed composer settles and releases its document",false);finish();return;}
        QTimer::singleShot(50,this,[this,next,attempts]{afterClosed(next,attempts-1);});
    }
    void completeRecipient(){stage=18;auto to=window->findChild<QQuickItem*>("composeTo");to->setProperty("text",QString("\"Saved, Person\" <saved@example.com>, May"));to->setProperty("cursorPosition",to->property("text").toString().size());to->forceActiveFocus();QMetaObject::invokeMethod(to,"query");}
    void choose(const QString&account,const QString&address){QVariant row=QVariantMap{{"account",account},{"address",address}};QMetaObject::invokeMethod(card,"chooseSender",Q_ARG(QVariant,row));}
    void finish(){if(finished)return;finished=true;QJsonObject r{{"passed",!failed},{"checks",checks},{"syntheticData",true}};if(auto path=qEnvironmentVariable("LUMA_COMPOSER_REPORT");!path.isEmpty()){QSaveFile f(path);if(f.open(QIODevice::WriteOnly)){f.write(QJsonDocument(r).toJson());f.commit();}}qInfo().noquote()<<QJsonDocument(r).toJson(QJsonDocument::Compact);mail->send("settingsUpdate",{{"draft_mode","manual"}});mail->quit(failed?2:0);}
public:
    ComposerSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        const auto root=qEnvironmentVariable("LUMA_TEST_ROOT");
        // Seed only optional recipient metadata in this isolated synthetic root.
        if(!root.isEmpty()){const auto dir=root+"/config/composer";QDir().mkpath(dir);QSaveFile file(dir+"/recipients.json");if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(QJsonArray{QJsonObject{{"account","preview"},{"address","Maya Chen <maya@example.com>"},{"last_sent",QDateTime::currentSecsSinceEpoch()}}}).toJson());file.setPermissions(QFileDevice::ReadOwner|QFileDevice::WriteOwner);file.commit();}}
        connect(mail,&Bridge::ready,this,[this]{QTimer::singleShot(300,this,[this]{
            auto shell=window->findChild<QQuickItem*>("composer");check("idle composer has no rich document or editor controls",shell&&!shell->property("controlsLoaded").toBool()&&!window->findChild<RichDocument*>("composeDocument")&&!window->findChild<QQuickItem*>("composeBody"));
            auto button=window->findChild<QQuickItem*>("newMessageButton");if(!button||!button->isVisible()){check("native compose button is available",false);finish();return;}
            const auto local=button->mapToScene(QPointF(button->width()/2,button->height()/2));const QPointF global=window->mapToGlobal(local.toPoint());
            QMouseEvent down(QEvent::MouseButtonPress,local,global,Qt::LeftButton,Qt::LeftButton,Qt::NoModifier);QCoreApplication::sendEvent(window,&down);
            QMouseEvent up(QEvent::MouseButtonRelease,local,global,Qt::LeftButton,Qt::NoButton,Qt::NoModifier);QCoreApplication::sendEvent(window,&up);
        });});
        connect(mail,&Bridge::received,this,[this](const QString&event,const QVariantMap&data){
            if(finished)return;
            if(event=="composeOpened")document=window->findChild<RichDocument*>("composeDocument");
            if(event=="error"){check("no unexpected composer error: "+data.value("message").toString(),false);stage=99;finish();return;}
            if(event=="composeOpened"&&stage==0){
                stage=1;session=data["session"].toString();card=window->findChild<QQuickItem*>("composer");document=window->findChild<RichDocument*>("composeDocument");
                check("engine session loads the native form",card&&document&&!session.isEmpty());if(!card||!document){finish();return;}
                QTimer::singleShot(750,this,[this]{
                    auto to=window->findChild<QQuickItem*>("composeTo"),subject=window->findChild<QQuickItem*>("composeSubject"),body=window->findChild<QQuickItem*>("composeBody");
                    check("new compose focuses To",window->activeFocusItem()==to);key(Qt::Key_Tab);check("Tab skips formatting controls to Subject",window->activeFocusItem()==subject);key(Qt::Key_Tab);check("Tab from Subject enters body",window->activeFocusItem()==body);key(Qt::Key_Backtab,Qt::ShiftModifier);check("Shift Tab returns to Subject",window->activeFocusItem()==subject);
                    const auto original=mail->m_composer;mail->m_composer.remove("document");mail->m_composer.remove("replace");mail->m_composer["sourceMode"]=true;emit mail->composerChanged();
                    auto attach=window->findChild<QQuickItem*>("composeAttachFiles"),bold=window->findChild<QQuickItem*>("composeBold");
                    check("preserved HTML keeps formatting read-only while attachments remain usable",body->property("readOnly").toBool()&&attach&&attach->isEnabled()&&bold&&!bold->isEnabled());
                    mail->m_composer=original;mail->m_composer.remove("document");mail->m_composer.remove("replace");emit mail->composerChanged();
                    check("editable messages restore formatting without changing field focus",bold&&bold->isEnabled()&&!body->property("readOnly").toBool()&&window->activeFocusItem()==subject);
                    auto tableButton=window->findChild<QQuickItem*>("composeTable");auto tableDialog=window->findChild<QObject*>("composeTableDialog");
                    check("table insertion is available in the composer toolbar",tableButton&&tableButton->isEnabled()&&tableDialog);
                    if(tableButton&&tableDialog){QMetaObject::invokeMethod(tableButton,"clicked");check("table control opens its modal options",tableDialog->property("visible").toBool());QMetaObject::invokeMethod(tableDialog,"accept");}
                    auto table=QTextCursor(document->nativeDocument()->begin()).currentTable();
                    // The first block is outside the table; inspect the live editor cursor.
                    QTextCursor tableCursor(document->nativeDocument());tableCursor.setPosition(body->property("cursorPosition").toInt());table=tableCursor.currentTable();
                    check("table options insert a real two-by-two rich table",table&&table->rows()==2&&table->columns()==2);
                    if(table){
                        body->forceActiveFocus();key(Qt::Key_Tab);tableCursor.setPosition(body->property("cursorPosition").toInt());check("composer Tab moves to the next table cell",table->cellAt(tableCursor).column()==1&&table->cellAt(tableCursor).row()==0);
                        key(Qt::Key_Backtab,Qt::ShiftModifier);tableCursor.setPosition(body->property("cursorPosition").toInt());check("composer Shift Tab returns to the previous table cell",table->cellAt(tableCursor).column()==0&&table->cellAt(tableCursor).row()==0);
                    }
                    document->load({{"kind","plain"},{"text",""}});subject->forceActiveFocus();
                    to->setProperty("text","person@example.com");subject->setProperty("text","Synthetic composer test");QTextCursor c(document->nativeDocument());c.insertText("Hello synthetic world");document->selection(0,5,5);document->toggle("bold");
                    check("selected formatting produces rich outgoing markup",document->payload()["html"].toBool()&&document->html().contains("font-weight:700"));
                    if(auto path=qEnvironmentVariable("LUMA_COMPOSER_SCREENSHOT");!path.isEmpty())window->grabWindow().save(path);
                    invoke("sync");
                    // Supply a synthetic MIME source without replacing the user's OS clipboard.
                    QImage image(400,300,QImage::Format_RGB32);quint32 noise=17;for(int y=0;y<image.height();++y)for(int x=0;x<image.width();++x){noise=noise*1664525+1013904223;image.setPixel(x,y,0xff000000|(noise&0xffffff));}
                    QByteArray png;QBuffer buffer(&png);buffer.open(QIODevice::WriteOnly);image.save(&buffer,"PNG");
                    auto fixture=new QMimeData;fixture->setParent(this);fixture->setData("image/png",png);mail->clipboardFixture=fixture;stage=12;
                    check("large clipboard image crosses acknowledged chunks",png.size()>256*1024&&mail->interceptImagePaste());
                });
            }else if(event=="clipboardComplete"&&stage==12){stage=13;check("clipboard PNG is validated as a MIME attachment",data["ok"].toBool());}
            else if(event=="compose"&&stage==13&&!data["inputBusy"].toBool()){
                check("image paste leaves authored text outside image resources",document->nativeDocument()->toPlainText()=="Hello synthetic world"&&data["attachments"].toList().size()==1);
                QImage bitmap(2,2,QImage::Format_RGB32);bitmap.fill(Qt::cyan);mail->clipboardFixture->setData("image/png",QByteArray());mail->clipboardFixture->setImageData(bitmap);stage=14;mail->interceptImagePaste();
            }else if(event=="clipboardComplete"&&stage==14){stage=15;check("native clipboard bitmap encodes off the UI thread",data["ok"].toBool());}
            else if(event=="compose"&&stage==15&&!data["inputBusy"].toBool()){
                check("two pasted images remain attachments without a draft",data["attachments"].toList().size()==2&&data["draftId"].isNull());mail->clipboardFixture.clear();
                if(mail->settings().value("accounts").toList().size()>1){stage=16;choose("preview-b","jamie@example.com");}else completeRecipient();
            }else if(event=="compose"&&stage==16&&data["account"]=="preview-b"){
                check("From switches the owning account without resetting body",data["from"]=="jamie@example.com"&&document->nativeDocument()->toPlainText()=="Hello synthetic world");stage=17;choose("preview","alex@example.com");
            }else if(event=="compose"&&stage==17&&data["account"]=="preview"){
                check("switching back preserves attachments and verified From",data["attachments"].toList().size()==2&&data["senderVerified"].toBool());completeRecipient();
            }else if(event=="recipients"&&stage==18&&data["prefix"]=="May"){
                auto to=window->findChild<QQuickItem*>("composeTo");check("recipient suggestions respect quoted display-name commas",to->property("suggestionsOpen").toBool()&&data["items"].toList().size()==1);
                key(Qt::Key_Tab);check("Tab completes a recipient and stays in the field",window->activeFocusItem()==to&&to->property("text").toString()=="\"Saved, Person\" <saved@example.com>, Maya Chen <maya@example.com>, ");
                key(Qt::Key_Tab);check("next Tab resumes the text-field navigation chain",window->activeFocusItem()==window->findChild<QQuickItem*>("composeSubject"));stage=1;invoke("sync");invoke("requestClose");
            }else if(event=="composeCloseQuestion"&&stage==1){
                stage=2;check("close asks before any Gmail draft write",data["revision"].toULongLong()>0);
                QMetaObject::invokeMethod(card,"answerClose",Q_ARG(QVariant,QString("keep")));
                QTimer::singleShot(200,this,[this]{mail->requestCompose("reply");});
            }else if(event=="composeFocus"&&stage==2){
                stage=3;check("repeated compose focuses the same session",data["session"]==session&&!card->property("expanded").toBool());mail->quit();
            }else if(event=="composeCloseQuestion"&&stage==3){
                stage=4;check("quit uses the same draft close policy",data["session"]==session);QMetaObject::invokeMethod(card,"answerClose",Q_ARG(QVariant,QString("keep")));
            }else if(event=="quitCancelled"&&stage==4){
                stage=5;check("keep editing cancels quit",card->property("shown").toBool());mail->send("settingsUpdate",{{"draft_mode","manual"}});
            }else if(event=="settings"&&stage==5){stage=6;invoke("requestClose");}
            else if(event=="composeClosed"&&stage==6){stage=7;check("manual close clears engine content",data["quote"].toString().isEmpty()&&data["attachments"].toList().isEmpty());afterClosed([this]{check("close releases the native rich document",!document&&!card->property("controlsLoaded").toBool());mail->selectRow(0,0,240,100,450,68);});}
            else if(event=="body"&&stage==7){stage=8;mail->requestCompose("reply",mail->messages()->at(0));}
            else if(event=="composeOpened"&&stage==8){
                stage=9;session=data["session"].toString();check("reply has read-only history outside the editor",!data["quote"].toString().isEmpty()&&(!document||document->nativeDocument()->isEmpty()));
                check("reply keeps its account ownership",data["accountLocked"].toBool());
                QTimer::singleShot(750,this,[this]{check("reply docks inside the reading thread",card->property("inlineMode").toBool()&&!card->property("expanded").toBool());check("reply focuses authored body",window->activeFocusItem()==window->findChild<QQuickItem*>("composeBody"));mail->requestCompose("reply",mail->messages()->at(0));});
            }else if(event=="composeFocus"&&stage==9){stage=10;check("repeated reply never maximizes or creates a draft",data["session"]==session&&card->property("inlineMode").toBool()&&!card->property("expanded").toBool());invoke("requestClose");}
            else if(event=="composeClosed"&&stage==10){stage=11;afterClosed([this]{check("inline close collapses and releases the surface",!card->isVisible());finish();});}
        });
        QTimer::singleShot(22000,this,[this]{if(!finished){check("composer smoke completes within deadline",false);finish();}});
    }
};
void startComposerSmoke(Bridge*m,QQuickWindow*w){new ComposerSmoke(m,w);}
