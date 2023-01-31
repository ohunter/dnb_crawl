@echo off
pyinstaller -F main.py
pyinstaller -F generate.py

"C:\Program Files\7-Zip\7z.exe" a "./windows.zip" "./dist/*" "./Readme.md" "./config.yaml" "./drivers/windows/geckodriver.exe"
rmdir /S /Q build
rmdir /S /Q dist
del *.spec

pause