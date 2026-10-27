#pragma once
#include <QAbstractListModel>
#include <QJsonArray>
#include <QJsonObject>
#include <QHash>
#include <QVariantMap>
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
    void trimBodies(const QString&keep);
    void mergeMessages(const QJsonArray&items);
    int indexFor(const QString&key)const;
    static QString key(const QJsonObject &row);
signals: void countChanged();
private: void reindex();QList<QJsonObject> rows;QHash<QString,int> positions;
};

