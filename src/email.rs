use handlebars::Handlebars;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};

use crate::{config::Config, model::User};
use std::error::Error;

pub struct Email {
    user: User,
    url: String,
    from: String,
    config: Config,
}

impl Email {
    pub fn new(user: User, url: String, config: Config) -> Self {
        let from = format!("Codevo <{}>", config.smtp_from.to_owned());

        Email {
            user,
            url,
            from,
            config,
        }
    }

    fn new_transport(
        &self,
    ) -> Result<AsyncSmtpTransport<Tokio1Executor>, lettre::transport::smtp::Error> {
        let creds = Credentials::new(
            self.config.smtp_user.to_owned(),
            self.config.smtp_pass.to_owned(),
        );

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(
            &self.config.smtp_host.to_owned(),
        )?
        .port(self.config.smtp_port)
        .credentials(creds)
        .build();

        Ok(transport)
    }

    fn render_template(&self, template_name: &str) -> Result<String, handlebars::RenderError> {
        let mut handlebars = Handlebars::new();
        handlebars
            .register_template_file(template_name, &format!("./templates/{}.hbs", template_name))?;
        handlebars.register_template_file("styles", "./templates/partials/styles.hbs")?;
        handlebars.register_template_file("base", "./templates/layouts/base.hbs")?;

        let data = serde_json::json!({
            "first_name": &self.user.name.split_whitespace().next().unwrap(),
            "subject": &template_name,
            "url": &self.url
        });

        let content_template = handlebars.render(template_name, &data)?;

        Ok(content_template)
    }

    async fn send_email(
        &self,
        template_name: &str,
        subject: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let html_template = self.render_template(template_name)?;
        let email = Message::builder()
            .to(
                format!("{} <{}>", self.user.name.as_str(), self.user.email.as_str())
                    .parse()
                    .unwrap(),
            )
            .reply_to(self.from.as_str().parse().unwrap())
            .from(self.from.as_str().parse().unwrap())
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html_template)?;

        let transport = self.new_transport()?;

        transport.send(email).await?;
        Ok(())
    }

    pub async fn send_verification_code(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_email("verification_code", "Your account verification code")
            .await
    }

    pub async fn send_password_reset_token(
        &self,
        password_reset_token_expires_in: i64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.send_email(
            "reset_password",
            format!(
                "Your password reset token (valid for only {} minutes)",
                password_reset_token_expires_in
            )
            .as_str(),
        )
        .await
    }
}

pub async fn send_password_reset_email(email: &str, token: &str, config: &Config) -> Result<(), Box<dyn Error>> {
    let reset_link = format!("http://localhost:3000/pwd-reset/{}", token);

    // Create the email content
    let email_content = format!(
        "<p>Hi,</p>
        <p>You requested a password reset. Click the link below to reset your password:</p>
        <p><a href=\"{}\">Reset Password</a></p>
        <p>If you did not request this, please ignore this email.</p>",
        reset_link
    );

    // Create the email message
    let email_message = Message::builder()
        .from(config.smtp_from.parse()?)
        .to(email.parse()?)
        .subject("Password Reset Request")
        .header(ContentType::TEXT_HTML)
        .body(email_content)?;

    // Set up the SMTP transport
    let creds = Credentials::new(config.smtp_user.clone(), config.smtp_pass.clone());
    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)?
        .port(config.smtp_port)
        .credentials(creds)
        .build();

    // Send the email
    mailer.send(email_message).await?;
    Ok(())
}

