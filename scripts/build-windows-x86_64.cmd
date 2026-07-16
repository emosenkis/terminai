FOR /F "usebackq tokens=*" %%i IN (`powershell -NoProfile -Command "$m = cargo metadata --format-version 1 --no-deps ^| ConvertFrom-Json; ($m.packages ^| Where-Object { $_.name -eq 'termin' }).version"`) DO SET VERSION=%%i

MKDIR release\terminai-%VERSION%-windows-x86_64 || exit /b

:: Windows x64

cargo build --release --bin terminai || exit /b

COPY target\release\terminai.exe release\terminai-%VERSION%-windows-x86_64\terminai.exe || exit /b

tar.exe -a -c -f release\terminai-%VERSION%-windows-x86_64.zip -C release\terminai-%VERSION%-windows-x86_64 terminai.exe
