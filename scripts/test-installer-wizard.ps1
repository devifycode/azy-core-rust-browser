$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$info=Get-Content -LiteralPath (Join-Path $projectRoot 'BUILD-INFO.json') -Raw | ConvertFrom-Json
$installer=Join-Path $projectRoot ('AzyCore-Setup-'+$info.version+'-x64.exe')
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
$stage=Join-Path $projectRoot '.cache\installer-build'
New-Item -ItemType Directory -Force -Path $stage,$env:TEMP | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class AzySetupProbe {
 public delegate bool EnumProc(IntPtr hwnd,IntPtr arg);
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd,EnumProc proc,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc,IntPtr arg);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll")] static extern int GetDlgCtrlID(IntPtr hwnd);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder text,int length);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd,StringBuilder text,int length);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr hwnd,uint message,IntPtr wp,string lp);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageW")] static extern IntPtr ReadMessage(IntPtr hwnd,uint message,IntPtr wp,StringBuilder text);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd,IntPtr hdc,uint flags);
 public static IntPtr Find(IntPtr root,int id,string kind) {
  IntPtr result=IntPtr.Zero;
  EnumChildWindows(root,(hwnd,arg)=>{var c=new StringBuilder(100);GetClassName(hwnd,c,100);if(GetDlgCtrlID(hwnd)==id&&c.ToString()==kind){result=hwnd;return false;}return true;},IntPtr.Zero);return result;
 }
 public static IntPtr Root(uint process){IntPtr result=IntPtr.Zero;EnumWindows((hwnd,arg)=>{uint pid;GetWindowThreadProcessId(hwnd,out pid);var c=new StringBuilder(100);GetClassName(hwnd,c,100);if(pid==process&&c.ToString()=="#32770"){result=hwnd;return false;}return true;},IntPtr.Zero);return result;}
 public static string Text(IntPtr hwnd){var s=new StringBuilder(4096);ReadMessage(hwnd,0xD,new IntPtr(s.Capacity),s);return s.ToString();}
}
'@
$p=Start-Process -FilePath $installer -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
try {
 $deadline=[DateTime]::UtcNow.AddSeconds(15)
 do {$hwnd=[AzySetupProbe]::Root($p.Id);if($hwnd -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow -lt $deadline)
 if($hwnd -eq [IntPtr]::Zero){throw 'Installer wizard did not open'}
 $next=[AzySetupProbe]::Find($hwnd,1,'Button');if($next -eq [IntPtr]::Zero){throw 'Welcome Next button missing'}
 [void][AzySetupProbe]::SendMessage($next,0xF5,[IntPtr]::Zero,$null)
 $deadline=[DateTime]::UtcNow.AddSeconds(10)
 do {$edit=[AzySetupProbe]::Find($hwnd,1019,'Edit');if($edit -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow -lt $deadline)
 if($edit -eq [IntPtr]::Zero){throw 'Directory chooser field missing'}
 $browse=[AzySetupProbe]::Find($hwnd,1001,'Button');if($browse -eq [IntPtr]::Zero){throw 'Directory Browse button missing'}
 $selected=Join-Path $stage 'Pilihan Folder Pengguna ü\Azy Core'
 [void][AzySetupProbe]::SendMessage($edit,0xC,[IntPtr]::Zero,$selected)
 if([AzySetupProbe]::Text($edit) -ne $selected){throw 'Wizard did not accept custom directory text'}
 Start-Sleep -Milliseconds 200
 $rect=[AzySetupProbe+Rect]::new();[void][AzySetupProbe]::GetWindowRect($hwnd,[ref]$rect)
 $bitmap=[Drawing.Bitmap]::new(($rect.Right-$rect.Left),($rect.Bottom-$rect.Top))
 $graphics=[Drawing.Graphics]::FromImage($bitmap);$hdc=$graphics.GetHdc()
 try{[void][AzySetupProbe]::PrintWindow($hwnd,$hdc,2)}finally{$graphics.ReleaseHdc($hdc);$graphics.Dispose()}
 try{$bitmap.Save((Join-Path $stage 'installer-directory.png'),[Drawing.Imaging.ImageFormat]::Png)}finally{$bitmap.Dispose()}
 Write-Host 'PASS Wizard shows directory input and Browse button, and accepts a custom path'
}finally{if(-not $p.HasExited){Stop-Process -Id $p.Id}}
