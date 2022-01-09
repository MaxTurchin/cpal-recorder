$asio_archive = "$pwd\asio_sdk.zip"
$asio_sdk_dir = "$pwd\.asio_sdk"

$llvm_installer = "$pwd\llvm_installer.exe"
$llvm_path = "C:\Program Files\LLVM\bin"

$vcvarsall_path = "C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\vcvarsall.bat"

Function InstallSources {
    echo "Downloading asio_sdk to $asio_archive..."
    wget https://www.steinberg.net/asiosdk -OutFile $asio_archive

    echo "Extracting $asio_archive..."
    Expand-Archive $asio_archive
    mv $($asio_archive -replace '.zip', '') $asio_sdk_dir

    echo "Downloading llvm_installer to $llvm_installer"
    echo "This might take a few minutes"

    wget https://github.com/llvm/llvm-project/releases/download/llvmorg-13.0.0/LLVM-13.0.0-win64.exe -OutFile $llvm_installer
    echo "Running $llvm_installer..."
    Start-Process $llvm_installer -Wait
}

Function SetEnv {
    echo "Setting environmnet variables..."
    [Environment]::SetEnvironmentVariable('CPAL_ASIO_DIR', "$asio_sdk_dir\$(ls $asio_sdk_dir)")
    [Environment]::SetEnvironmentVariable('LIBCLANG_PATH', "$llvm_path")
    [Environment]::SetEnvironmentVariable('amd64', "$vcvarsall_path")
    echo "Done."
}

Function CleanUp {
    echo "Cleaning up..."
    Remove-Item $asio_archive
    Remove-Item $llvm_installer
}

InstallSources
SetEnv
CleanUp
