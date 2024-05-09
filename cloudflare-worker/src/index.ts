import { Buffer } from 'node:buffer';

interface Mail {
	/**
		* The raw contents of the e-mail, encoded with base64.
		*/
	raw: string,

	/**
		* The size, in bytes, of the raw e-mail message, before base64 encoding.
		*/
	raw_size: number;

	/**
		* Information about the e-mail that may be known prior to actual parsing of
		* the e-mail.
		*/
	metadata?: MailMetadata;
}

interface MailMetadata {
	/**
		* The intended recipient of the e-mail, if any.
		*/
	to?: string;

	/**
		* The sender of the e-mail, if any.
		*/
	from?: string;

	/**
		* E-mail headers, if any.
		*/
	headers?: { [k: string]: string };
}

interface MailIngestionRequest {
	/**
		* The list of mails to ingest.
		*/
	mails: Array<Mail>;

	/**
		* The time when we started processing e-mails.
		*/
	started_at: Date;
}

async function streamToBase64String(stream: ReadableStream) {
	// lets have a ReadableStream as a stream variable
	const chunks = [];

	for await (const chunk of stream) {
		chunks.push(Buffer.from(chunk));
	}

	return Buffer.concat(chunks).toString("base64");
}

export default {
	async email(message: ForwardableEmailMessage, env: Env, _ctx: ExecutionContext) {
		const dt = new Date();
		const headers = {
			"Content-Type": "application/json",
			"Authorization": `Token ${env.API_TOKEN}`
		};

		const payload: MailIngestionRequest = {
			mails: [<Mail>{
				metadata: {
					to: message.to,
					from: message.from,
					headers: Object.fromEntries(message.headers.entries())
				},
				raw: await (streamToBase64String(message.raw)),
				raw_size: message.rawSize
			}],
			started_at: dt
		};

		const API_URL = `${env.SERVICE_URL}/api/v1/ingestion`;
		console.log("API_URL = %s", API_URL);
		// console.log(JSON.stringify(headers));
		// console.log(JSON.stringify(payload));

		const result = await fetch(API_URL, {
			headers,
			method: "POST",
			body: JSON.stringify(payload),
		});

		console.log("result:");
		console.log(await result.text());
	}
}
