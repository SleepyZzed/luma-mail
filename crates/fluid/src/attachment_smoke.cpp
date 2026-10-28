#include "attachment_preview.h"
#include "bridge.h"
#include "attachment_fixture.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QDir>
#include <QFile>
#include <QSaveFile>
#include <QJsonDocument>
#include <QMetaEnum>
#include <QtEndian>
#include <QMimeData>
#include <QDragEnterEvent>
#include <QDragMoveEvent>
#include <QDropEvent>
#include <QQuickTextDocument>
#include <QTextDocument>
class AttachmentSmoke:public QObject {
    Bridge*mail;AttachmentPreview*preview;QQuickWindow*window;QJsonArray checks;QTimer poll;bool failed=false,finished=false;int stage=0;QString session;QVariantMap imageSource,textSource,audioSource,videoSource;
    QString thumbKey;
    QQuickItem*find(QQuickItem*at,const QString&name){if(at->objectName()==name)return at;for(auto child:at->childItems())if(auto item=find(child,name))return item;return nullptr;}
    void check(const QString&name,bool value){checks.append(QJsonObject{{"check",name},{"passed",value}});failed|=!value;}
    void finish(){if(finished)return;finished=true;poll.stop();preview->close();QJsonObject report{{"passed",!failed},{"checks",checks},{"syntheticData",true},{"note","Only explicit synthetic input fixtures are written. No mail cache, spool or local draft is created."}};if(auto path=qEnvironmentVariable("LUMA_ATTACHMENT_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(report).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(report).toJson(QJsonDocument::Compact);mail->quit(failed?2:0);}
    void closePane(){if(auto pane=window->findChild<QQuickItem*>("attachmentPane"))QMetaObject::invokeMethod(pane,"close");}
    void inputs(){
        auto path=qEnvironmentVariable("LUMA_TEST_ROOT")+"/inputs";if(!QDir().mkpath(path)){check("synthetic input directory is available",false);finish();return;}QImage image(160,100,QImage::Format_RGBA8888);image.fill(QColor("#886ec7"));if(!image.save(path+"/image.png")){check("synthetic image writes successfully",false);finish();return;}QFile text(path+"/notes.txt");if(!text.open(QIODevice::WriteOnly)){check("synthetic text fixture opens",false);finish();return;}text.write("A synthetic attachment, previewed entirely in RAM.");text.close();
        QByteArray wave;auto u32=[&](quint32 n){char out[4];qToLittleEndian(n,out);wave.append(out,4);};auto u16=[&](quint16 n){char out[2];qToLittleEndian(n,out);wave.append(out,2);};const quint32 samples=320000;
        wave.append("RIFF");u32(samples+36);wave.append("WAVEfmt ");u32(16);u16(1);u16(1);u32(8000);u32(8000);u16(1);u16(8);wave.append("data");u32(samples);wave.append(QByteArray(samples,char(128)));QFile audio(path+"/tone.wav");if(!audio.open(QIODevice::WriteOnly)){check("synthetic audio fixture opens",false);finish();return;}audio.write(wave);audio.close();
        QFile video(path+"/video.mp4");if(!video.open(QIODevice::WriteOnly)){check("synthetic video fixture opens",false);finish();return;}video.write(QByteArray::fromBase64(AttachmentVideo));video.close();
        check("file drops reject remote URLs without creating attachments",!mail->attachUrls({QUrl("https://example.invalid/file")})&&mail->composer()["attachments"].toList().isEmpty());mail->clearError();
        QVariantList tooMany;for(int i=0;i<65;++i)tooMany.append(QUrl::fromLocalFile(path+"/notes.txt"));check("file-drop batches keep the 64-file bound",!mail->attachUrls(tooMany));mail->clearError();
        QTimer::singleShot(800,this,[this,path]{
            auto card=find(window->contentItem(),"composer");if(!card){check("file-drop composer exists",false);finish();return;}
            QMimeData mime;mime.setUrls({QUrl::fromLocalFile(path+"/image.png"),QUrl::fromLocalFile(path+"/notes.txt"),QUrl::fromLocalFile(path+"/tone.wav"),QUrl::fromLocalFile(path+"/video.mp4")});
            const auto point=card->mapToScene(QPointF(card->width()/2,card->height()/2));
            QDragEnterEvent enter(point.toPoint(),Qt::CopyAction|Qt::MoveAction,&mime,Qt::LeftButton,Qt::NoModifier);QCoreApplication::sendEvent(window,&enter);
            QDragMoveEvent move(point.toPoint(),Qt::CopyAction|Qt::MoveAction,&mime,Qt::LeftButton,Qt::NoModifier);QCoreApplication::sendEvent(window,&move);
            QDropEvent drop(point,Qt::CopyAction|Qt::MoveAction,&mime,Qt::LeftButton,Qt::NoModifier);drop.setDropAction(Qt::MoveAction);QCoreApplication::sendEvent(window,&drop);
            check("native file drops are accepted as Copy and never Move",enter.isAccepted()&&drop.isAccepted()&&drop.dropAction()==Qt::CopyAction);
        });
    }
public:
    AttachmentSmoke(Bridge*m,AttachmentPreview*p,QQuickWindow*w):QObject(w),mail(m),preview(p),window(w){
        QTimer::singleShot(20000,this,[this]{if(!finished){check("attachment test completes within deadline: "+preview->error(),false);finish();}});
        connect(mail,&Bridge::ready,this,[this]{mail->send("settingsUpdate",{{"draft_mode","manual"}});QTimer::singleShot(250,this,[this]{mail->requestCompose();});});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&data){if(finished)return;if(kind=="error"){check("attachment commands succeed: "+data.value("message").toString(),false);finish();return;}
            if(kind=="composeOpened"&&stage==0){stage=1;session=data["session"].toString();inputs();}
            else if(kind=="compose"&&stage==1&&data["attachments"].toList().size()==4){
                auto body=find(window->contentItem(),"composeBody");auto document=body?qvariant_cast<QQuickTextDocument*>(body->property("textDocument")):nullptr;check("file drops preserve authored text instead of inserting file URLs",document&&document->textDocument()->toPlainText().isEmpty());
                stage=2;auto rows=data["attachments"].toList();auto owner=QVariantMap{{"session",session},{"generation",data["attachmentGeneration"]}};
                imageSource=owner;imageSource["attachment"]=rows[0].toMap()["id"];textSource=owner;textSource["attachment"]=rows[1].toMap()["id"];audioSource=owner;audioSource["attachment"]=rows[2].toMap()["id"];
                videoSource=owner;videoSource["attachment"]=rows[3].toMap()["id"];
                thumbKey=preview->watchThumbnail(imageSource,"image.png","image/png",rows[0].toMap()["size"].toLongLong());
                preview->open(imageSource,"image.png",QRectF(450,170,235,66));
            }
        });
        poll.setInterval(100);connect(&poll,&QTimer::timeout,this,[this]{if(finished)return;
            if(preview->opened()&&!preview->loading()&&!preview->error().isEmpty()){check("native preview succeeds: "+preview->error(),false);finish();return;}
            if(stage==2&&!preview->loading()&&preview->kind()=="Image"&&!preview->thumbnailSource(thumbKey).isEmpty()){
                stage=3;check("image preview uses the native RAM provider",!preview->image().isEmpty()&&preview->pixels()->image.size()==QSize(160,100));
                QTimer::singleShot(800,this,[this]{auto thumbnail=find(window->contentItem(),"attachmentThumbnail-"+thumbKey);check("visible compose attachment shows its RAM thumbnail",thumbnail&&thumbnail->property("status").toInt()==1);check("thumbnail pixels stay inside the 1.5 MiB budget",preview->thumbnailBytes()>0&&preview->thumbnailBytes()<=32*128*96*4);auto pane=window->findChild<QQuickItem*>("attachmentPane");check("attachment card morph reaches its expanded bounds",pane&&pane->property("shown").toBool()&&!pane->property("moving").toBool());if(auto path=qEnvironmentVariable("LUMA_ATTACHMENT_SCREENSHOT");!path.isEmpty())window->grabWindow().save(path);closePane();stage=4;});
            }else if(stage==4&&!preview->opened()){
                stage=5;check("closing releases decoded preview pixels",preview->pixels()->image.isNull());QTimer::singleShot(100,this,[this]{preview->open(textSource,"notes.txt",QRectF(450,170,235,66));});
            }else if(stage==5&&preview->opened()&&!preview->loading()&&preview->kind()=="Text"){
                stage=6;check("document text is displayed without extraction",preview->text().contains("entirely in RAM"));closePane();
            }else if(stage==6&&!preview->opened()){
                stage=7;QTimer::singleShot(100,this,[this]{preview->open(audioSource,"tone.wav",QRectF(450,170,235,66));});
            }else if(stage==7&&preview->duration()>39000){
                stage=8;check("chunked audio loads directly from a memory device",!preview->loading()&&preview->kind()=="Audio");preview->seek(1000);check("media supports seeking",preview->position()>=1000);preview->playPause();
                QTimer::singleShot(200,this,[this]{check("playback starts without a temporary media file",preview->playing());preview->pause();closePane();stage=9;});
            }else if(stage==9&&!preview->opened()){
                check("close releases the player and media buffer",preview->duration()==0&&preview->position()==0);stage=10;QTimer::singleShot(100,this,[this]{preview->open(videoSource,"video.mp4",QRectF(450,170,235,66));});
            }else if(stage==10&&preview->duration()>=900){stage=11;check("video opens from the same bounded RAM path",preview->kind()=="Video"&&!preview->loading());preview->playPause();
            }else if(stage==11&&preview->hasVideoFrame()){stage=12;check("video frames reach the native video sink",true);closePane();
            }else if(stage==12&&!preview->opened()){
                stage=13;preview->releaseThumbnail(thumbKey);window->hide();QTimer::singleShot(200,this,[this]{check("hiding the window releases thumbnails and the last video frame",preview->thumbnailBytes()==0&&!preview->hasVideoFrame());const auto root=qEnvironmentVariable("LUMA_TEST_ROOT");check("previewing creates no mail cache or local draft",!QFileInfo::exists(root+"/cache/bodies")&&!QFileInfo::exists(root+"/config/composer"));finish();});
            }
        });poll.start();
    }
};
void startAttachmentSmoke(Bridge*m,AttachmentPreview*p,QQuickWindow*w){new AttachmentSmoke(m,p,w);}
