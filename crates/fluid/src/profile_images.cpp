#include "profiles.h"
#include <QMutexLocker>
#include <QPainter>
#include <QPainterPath>
#include <QUuid>
#include <limits>
#include <algorithm>
QImage ProfileImages::requestImage(const QString&id,QSize*size,const QSize&){QMutexLocker lock(&mutex);auto image=images.value(id);if(size)*size=image.size();return image;}
bool ProfileImages::insert(const QString&id,const QByteArray&rgba,int width,int height){
    if(id.isEmpty()||width<1||height<1||width>96||height>96||rgba.size()!=width*height*4)return false;
    QMutexLocker lock(&mutex);if(!images.contains(id)&&images.size()>=128)return false;
    QImage source(reinterpret_cast<const uchar*>(rgba.constData()),width,height,QImage::Format_RGBA8888);
    QImage circle(96,96,QImage::Format_ARGB32_Premultiplied);circle.fill(Qt::transparent);
    QPainter painter(&circle);painter.setRenderHint(QPainter::Antialiasing);QPainterPath path;path.addEllipse(QRectF(0,0,96,96));painter.setClipPath(path);
    auto scaled=source.scaled(96,96,Qt::KeepAspectRatioByExpanding,Qt::SmoothTransformation);painter.drawImage(QPoint((96-scaled.width())/2,(96-scaled.height())/2),scaled);painter.end();images.insert(id,circle);return true;
}
bool ProfileImages::contains(const QString&id)const{QMutexLocker lock(&mutex);return images.contains(id);}
void ProfileImages::remove(const QString&id){QMutexLocker lock(&mutex);images.remove(id);}
void ProfileImages::clear(){QMutexLocker lock(&mutex);images.clear();}
int ProfileImages::count()const{QMutexLocker lock(&mutex);return images.size();}
qint64 ProfileImages::bytes()const{QMutexLocker lock(&mutex);qint64 total=0;for(const auto&i:images)total+=i.sizeInBytes();return total;}
