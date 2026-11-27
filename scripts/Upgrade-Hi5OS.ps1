param([Parameter(Mandatory=$true)][string]$VmFolder)
$ErrorActionPreference='Stop'
$src=$PSScriptRoot
$dst=(Resolve-Path -LiteralPath $VmFolder).Path
if ($src -eq $dst) { throw 'Extract upgrade into a separate folder first.' }
if (Get-Process -Name vmware,vmware-vmx -ErrorAction SilentlyContinue) { throw 'Fully power off all VMware guests and close Workstation first.' }
if (Get-ChildItem -LiteralPath $dst -Force | Where-Object { $_.Name -match '\.lck$|\.vmss$|\.vmsn$|-\d{6}(-delta|-sesparse)?\.vmdk$' }) { throw 'Lock, suspended state or snapshot detected. Resolve through VMware before upgrading.' }
$vmx=@(Get-ChildItem -LiteralPath $dst -Filter *.vmx)
if ($vmx.Count -ne 1) { throw 'Expected exactly one VMX in the selected folder.' }
if ((Get-Content -LiteralPath $vmx[0].FullName -Raw) -notmatch '(?im)^sata0:0\.fileName\s*=\s*"Hi5OS-system\.vmdk"\s*$') { throw 'Unsupported VMX disk layout; no files changed.' }
$names=@('Hi5OS-system.vmdk','Hi5OS-system-flat.vmdk')
$manifest=Get-Content -LiteralPath (Join-Path $src 'manifest.json') -Raw | ConvertFrom-Json
if ($manifest.schema -ne 1) { throw 'Unsupported manifest.' }
foreach ($name in $names) {
  $p=Join-Path $src $name
  if ((Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash -ne $manifest.files.$name) { throw "Hash mismatch: $name" }
  if (!(Test-Path -LiteralPath (Join-Path $dst $name) -PathType Leaf)) { throw "Missing existing system file: $name" }
}
if ((Get-Item -LiteralPath (Join-Path $dst 'Hi5OS-system-flat.vmdk')).Length -ne 134217728) { throw 'Unsupported system extent size.' }
$answer=Read-Host 'Confirm VM is powered off, snapshots consolidated and full VM backup exists. Type UPGRADE'
if ($answer -cne 'UPGRADE') { throw 'Cancelled.' }
$backup=Join-Path $dst ('Hi5OS-system-backup-'+(Get-Date -Format yyyyMMdd-HHmmss))
New-Item -ItemType Directory -Path $backup | Out-Null
foreach ($name in $names) { Copy-Item -LiteralPath (Join-Path $dst $name) -Destination (Join-Path $backup $name) }
try {
  foreach ($name in $names) {
    $temp=Join-Path $dst ($name+'.hi5new')
    if (Test-Path -LiteralPath $temp) { throw "Staging file exists: $temp" }
    Copy-Item -LiteralPath (Join-Path $src $name) -Destination $temp
    if ((Get-FileHash -LiteralPath $temp -Algorithm SHA256).Hash -ne $manifest.files.$name) { throw "Staged hash mismatch: $name" }
  }
  foreach ($name in $names) { Move-Item -LiteralPath (Join-Path $dst ($name+'.hi5new')) -Destination (Join-Path $dst $name) -Force }
  foreach ($name in $names) { if ((Get-FileHash -LiteralPath (Join-Path $dst $name) -Algorithm SHA256).Hash -ne $manifest.files.$name) { throw "Installed hash mismatch: $name" } }
} catch {
  foreach ($name in $names) { Copy-Item -LiteralPath (Join-Path $backup $name) -Destination (Join-Path $dst $name) -Force }
  throw
}
Write-Host "Upgrade installed. System backup: $backup. VMX and data disk unchanged. Open existing VM in VMware."
