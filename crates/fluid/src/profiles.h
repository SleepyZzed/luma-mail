#pragma once
#include <QQuickImageProvider>
#include <QHash>
#include <QImage>
#include <QMutex>
class ProfileImages:public QQuickImageProvider {
public:
    ProfileImages():QQuickImageProvider(QQuickImageProvider::Image){}
    QImage requestImage(const QString&id,QSize*size,const QSize&requestedSize)override;
    bool insert(const QString&id,const QByteArray&rgba,int width,int height);
    bool contains(const QString&id)const;
    void remove(const QString&id);void clear();int count()const;qint64 bytes()const;
private:mutable QMutex mutex;QHash<QString,QImage> images;
};
