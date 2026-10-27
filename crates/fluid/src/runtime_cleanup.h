#pragma once
#include <QTemporaryDir>
#include <QLockFile>
#include <memory>
// Browser runtime scaffolding only. Email documents never enter this directory.
class ReaderRuntime {
public:
    ReaderRuntime();~ReaderRuntime();
    bool isValid()const;QString path()const;
    static void recover();
private:
    std::unique_ptr<QTemporaryDir> directory;std::unique_ptr<QLockFile> lock;
};
