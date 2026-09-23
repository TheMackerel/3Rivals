# Contribuire

3Rivals è nella fase 1: esiste la cintura, il resto è in costruzione. Per
ora il modo più utile di contribuire è **aprire una issue**.

## Segnalare un falso richiamo

Un falso richiamo — la cintura ti richiama mentre stai lavorando — è il difetto
più grave che può avere, e il più utile da segnalare. Allega la riga del
registro (`%APPDATA%\3Rivals\registro\AAAA-MM-GG.jsonl`) che l'ha provocato.

> **Prima di incollarla, guardala.** Il registro contiene i **titoli delle tue
> finestre**: nomi di file, di pagine, di chat. Sostituisci con qualcosa di
> inventato tutto ciò che non vuoi rendere pubblico — conta il nome del
> processo e la *forma* del titolo, non il suo contenuto.

## Pull request

Aprile pure, ma prima parliamone in una issue: il progetto segue un piano di
lavoro con criteri di accettazione, e una PR fuori piano rischia di non entrare.
Ogni PR deve far passare `cargo test --workspace`, e ogni numero che scrive in
un documento deve venire da un test, un log o una misura.

## Licenza dei contributi

Salvo indicazione contraria, ogni contributo inviato a questo repo è concesso
sotto la stessa doppia licenza del progetto, MIT OR Apache-2.0, senza termini
aggiuntivi.
